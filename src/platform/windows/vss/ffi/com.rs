//! Native COM API implementation for Windows VSS.
//!
//! Matches the C++ `Win32VSSWrapper/VssClient.cpp` workflow exactly.
//! Uses `windows` crate for COM infrastructure and raw vtables for VSS interfaces.

use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;

use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::types::{SnapshotHandle, SnapshotInfo, VolumeRef};

use super::super::BACKEND_NAME;

// ── VSS constants (from vss.h) ─────────────────────────────────────────────

const VSS_CTX_APP_ROLLBACK: i32 = 0x00000009;
#[allow(dead_code)]
const VSS_CTX_ALL: i32 = -1i32; // 0xFFFFFFFF

const VSS_BT_FULL: u32 = 1;
const VSS_OBJECT_SNAPSHOT: u32 = 3;
#[allow(dead_code)]
const VSS_OBJECT_NONE: u32 = 0;
#[allow(dead_code)]
const VSS_OBJECT_SNAPSHOT_SET: u32 = 2;

const VSS_S_ASYNC_FINISHED: i32 = 0x00042304;
const VSS_S_ASYNC_PENDING: i32 = 0x00042302;

// ── VSS_ID (GUID) ─────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VssId {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl VssId {
    const ZERO: Self = Self {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };

    pub fn to_guid_string(&self) -> String {
        format!(
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7],
        )
    }
}

// ── VSS property structs (from vss.h) ─────────────────────────────────────

/// VSS_SNAPSHOT_PROP — matches the Windows SDK struct layout.
#[repr(C)]
struct VssSnapshotProp {
    snapshot_id: VssId,
    snapshot_set_id: VssId,
    snapshots_count: i32,
    _pad1: [u8; 4],
    snapshot_device_object: *const u16,
    original_volume_name: *const u16,
    originating_machine: *const u16,
    service_machine: *const u16,
    exposed_name: *const u16,
    exposed_path: *const u16,
    provider_id: VssId,
    snapshot_attributes: i32,
    timestamp: i64,
    status: u32,
    _pad2: [u8; 4],
}

/// VSS_OBJECT_PROP — discriminant + union (only Snapshot variant used).
#[repr(C)]
struct VssObjectProp {
    object_type: u32,
    _pad: [u8; 4],
    snapshot: VssSnapshotProp,
}

// ── COM vtable function pointer types ──────────────────────────────────────

type QIFn = unsafe extern "system" fn(*mut c_void, *const VssId, *mut *mut c_void) -> i32;
type AddRefFn = unsafe extern "system" fn(*mut c_void) -> u32;
type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;

// ── IVssBackupComponents vtable ───────────────────────────────────────────
// Layout from Windows SDK vswriter.h, method indices match C++ VssClient.cpp.

#[repr(C)]
struct IVssBackupComponentsVtbl {
    // IUnknown (0-2)
    _query_interface: QIFn,
    _add_ref: AddRefFn,
    release: ReleaseFn,
    // IVssBackupComponents (3-25)
    _get_writer_metadata_count: *const c_void,
    _get_writer_metadata: *const c_void,
    _free_writer_metadata: *const c_void,
    _add_component: *const c_void,
    prepare_for_backup: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    add_to_snapshot_set:
        unsafe extern "system" fn(*mut c_void, *const u16, VssId, *mut VssId) -> i32,
    do_snapshot_set: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    _commit_snapshot_set: *const c_void,
    get_snapshot_properties:
        unsafe extern "system" fn(*mut c_void, VssId, *mut VssSnapshotProp) -> i32,
    _get_writer_status: *const c_void,
    set_backup_state: unsafe extern "system" fn(*mut c_void, bool, bool, u32, bool) -> i32,
    _set_backup_succeeded: *const c_void,
    _set_backup_options: *const c_void,
    _set_selected_for_restore: *const c_void,
    _set_restore_options: *const c_void,
    _backup_complete: *const c_void,
    _abort_backup: *const c_void,
    initialize_for_backup: unsafe extern "system" fn(*mut c_void) -> i32,
    set_context: unsafe extern "system" fn(*mut c_void, i32) -> i32,
    start_snapshot_set: unsafe extern "system" fn(*mut c_void, *mut VssId) -> i32,
    _import_snapshots: *const c_void,
    _break_snapshot_set: *const c_void,
    gather_writer_metadata: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    // IVssBackupComponents::Query is at index 26 (IUnknown 3 + 23 + this)
    query: unsafe extern "system" fn(*mut c_void, VssId, u32, u32, *mut *mut c_void) -> i32,
    _is_volume_supported: *const c_void,
    _disable_writer_classes: *const c_void,
    _enable_writer_classes: *const c_void,
    _disable_writer_instances: *const c_void,
    _expose_snapshot: *const c_void,
    delete_snapshots:
        unsafe extern "system" fn(*mut c_void, VssId, u32, bool, *mut i32, *mut VssId) -> i32,
}

// ── IVssAsync vtable ───────────────────────────────────────────────────────

#[repr(C)]
struct IVssAsyncVtbl {
    _query_interface: QIFn,
    _add_ref: AddRefFn,
    release: ReleaseFn,
    wait: unsafe extern "system" fn(*mut c_void) -> i32,
    query_status: unsafe extern "system" fn(*mut c_void, *mut i32, *mut u32) -> i32,
}

// ── IVssEnumObject vtable ──────────────────────────────────────────────────

#[repr(C)]
struct IVssEnumObjectVtbl {
    _query_interface: QIFn,
    _add_ref: AddRefFn,
    release: ReleaseFn,
    next: unsafe extern "system" fn(*mut c_void, u32, *mut VssObjectProp, *mut u32) -> i32,
    _skip: *const c_void,
    _reset: *const c_void,
    _clone: *const c_void,
}

// ── IVssCoordinator vtable ─────────────────────────────────────────────────
// CLSID_VSSCoordinator IS registered on Windows, unlike CVssBackupComponents.

#[repr(C)]
struct IVssCoordinatorVtbl {
    // IUnknown (0-2)
    _query_interface: QIFn,
    _add_ref: AddRefFn,
    release: ReleaseFn,
    // IVssCoordinator — empirically verified vtable layout on Windows 10 19045
    // Indices 3-10 are other coordinator methods (SetContext, Query, etc.)
    _pad3: *const c_void,
    _pad4: *const c_void,
    _pad5: *const c_void,
    _pad6: *const c_void,
    _pad7: *const c_void,
    _pad8: *const c_void,
    _pad9: *const c_void,
    _pad10: *const c_void,
    // Index 11: DeleteSnapshots
    delete_snapshots:
        unsafe extern "system" fn(*mut c_void, VssId, u32, bool, *mut i32, *mut VssId) -> i32,
}

// ── COM pointer wrapper ────────────────────────────────────────────────────

struct ComPtr {
    ptr: *mut c_void,
}

impl ComPtr {
    fn from_raw(ptr: *mut c_void) -> Self {
        Self { ptr }
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }

    fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// # Safety
    /// Caller must ensure `self.ptr` is a valid COM pointer to an `IVssBackupComponents`
    /// interface. The returned reference is valid as long as the COM object lives.
    unsafe fn backup_components_vtbl(&self) -> &IVssBackupComponentsVtbl {
        // SAFETY: COM vtable pointer is always the first pointer-sized field of the object.
        // Dereferencing to read the vtable pointer is safe if the COM object is valid.
        unsafe {
            let vtbl_ptr = *(self.ptr as *const *const IVssBackupComponentsVtbl);
            &*vtbl_ptr
        }
    }

    /// # Safety
    /// Caller must ensure `self.ptr` is a valid COM pointer to an `IVssAsync` interface.
    unsafe fn async_vtbl(&self) -> &IVssAsyncVtbl {
        // SAFETY: Same vtable layout convention as backup_components_vtbl.
        unsafe {
            let vtbl_ptr = *(self.ptr as *const *const IVssAsyncVtbl);
            &*vtbl_ptr
        }
    }

    /// # Safety
    /// Caller must ensure `self.ptr` is a valid COM pointer to an `IVssEnumObject` interface.
    unsafe fn enum_vtbl(&self) -> &IVssEnumObjectVtbl {
        // SAFETY: Same vtable layout convention.
        unsafe {
            let vtbl_ptr = *(self.ptr as *const *const IVssEnumObjectVtbl);
            &*vtbl_ptr
        }
    }

    /// # Safety
    /// Caller must ensure `self.ptr` is a valid COM pointer to an `IVssCoordinator` interface.
    unsafe fn coordinator_vtbl(&self) -> &IVssCoordinatorVtbl {
        // SAFETY: Same vtable layout convention.
        unsafe {
            let vtbl_ptr = *(self.ptr as *const *const IVssCoordinatorVtbl);
            &*vtbl_ptr
        }
    }

    /// Release the COM object. Safe to call multiple times (nulls the pointer after release).
    fn release(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: self.ptr is non-null and was obtained from a COM Create/CoCreateInstance
            // call, so it has a valid vtable with Release at index 2 (IUnknown).
            unsafe {
                let vtbl_ptr = *(self.ptr as *const *const IVssBackupComponentsVtbl);
                ((*vtbl_ptr).release)(self.ptr);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

impl Drop for ComPtr {
    fn drop(&mut self) {
        self.release();
    }
}

// SAFETY: COM objects with COINIT_MULTITHREADED are thread-safe.
unsafe impl Send for ComPtr {}
unsafe impl Sync for ComPtr {}

// ── Windows FFI (from windows crate) ──────────────────────────────────────

use windows::Win32::System::Com::{
    COINIT_MULTITHREADED, CoInitializeEx, CoInitializeSecurity, EOAC_DYNAMIC_CLOAKING,
    RPC_C_AUTHN_LEVEL, RPC_C_IMP_LEVEL,
};

// Raw extern for functions not in windows crate
unsafe extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *const c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    fn SysAllocString(psz: *const u16) -> *const u16;
    fn SysFreeString(bstr: *const u16);
}

// ── COM initialization (once) ──────────────────────────────────────────────

static COM_INIT_RESULT: OnceLock<std::result::Result<(), Error>> = OnceLock::new();

fn ensure_com_initialized() -> Result<()> {
    let result = COM_INIT_RESULT.get_or_init(|| {
        (|| -> Result<()> {
            unsafe {
                // CoInitializeEx
                let hr_init = CoInitializeEx(None, COINIT_MULTITHREADED);
                if hr_init.is_err() {
                    return Err(Error::Message {
                        message: format!("CoInitializeEx failed: {:?}", hr_init),
                    });
                }

                // CoInitializeSecurity — CRITICAL for VSS COM calls
                // Exact params from C++ VssClient.cpp line 596-606
                let hr_sec = CoInitializeSecurity(
                    None,                  // pSecDesc
                    -1,                    // cAuthSvc (-1 = default)
                    None,                  // asAuthSvc
                    None,                  // pReserved1
                    RPC_C_AUTHN_LEVEL(6),  // RPC_C_AUTHN_LEVEL_PKT_PRIVACY
                    RPC_C_IMP_LEVEL(3),    // RPC_C_IMP_LEVEL_IMPERSONATE
                    None,                  // pAuthList
                    EOAC_DYNAMIC_CLOAKING, // 0x40
                    None,                  // pReserved3
                );
                if hr_sec.is_err() {
                    return Err(Error::Message {
                        message: format!("CoInitializeSecurity failed: {:?}", hr_sec),
                    });
                }

                Ok(())
            }
        })()
    });

    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(Error::Message {
            message: format!("COM initialization failed: {}", e),
        }),
    }
}

// ── Dynamic loading fallback ───────────────────────────────────────────────

type CreateVssBackupComponentsInternalFn = unsafe extern "system" fn(*mut *mut c_void) -> i32;

fn create_backup_components_dyn() -> Result<ComPtr> {
    let name = wide_string("vssapi.dll");
    let handle = unsafe { LoadLibraryW(name.as_ptr()) };
    if handle.is_null() {
        return Err(Error::Message {
            message: "failed to load vssapi.dll".to_string(),
        });
    }

    let names: &[&[u8]] = &[
        b"CreateVssBackupComponentsInternal\0",
        b"?CreateVssBackupComponents@@YAJPEAPEAVIVssBackupComponents@@@Z\0",
    ];

    for name in names {
        let proc = unsafe { GetProcAddress(handle, name.as_ptr()) };
        if !proc.is_null() {
            let fn_ptr: CreateVssBackupComponentsInternalFn = unsafe { std::mem::transmute(proc) };
            let mut raw: *mut c_void = ptr::null_mut();
            let hr = unsafe { fn_ptr(&mut raw) };
            unsafe { FreeLibrary(handle) };
            if hr >= 0 && !raw.is_null() {
                return Ok(ComPtr::from_raw(raw));
            }
            return Err(Error::Message {
                message: format!("CreateVssBackupComponents returned 0x{:08X}", hr as u32),
            });
        }
    }

    unsafe { FreeLibrary(handle) };
    Err(Error::Message {
        message: "CreateVssBackupComponents not found in vssapi.dll".to_string(),
    })
}

// ── Create IVssBackupComponents ────────────────────────────────────────────

/// Create IVssBackupComponents by dynamically loading from vssapi.dll.
/// The export name varies across Windows editions (mangled C++ name on x64),
/// so static linking is not reliable.
fn create_backup_components() -> Result<ComPtr> {
    create_backup_components_dyn()
}

// ── Async wait ─────────────────────────────────────────────────────────────

/// Block until an IVssAsync operation completes.
///
/// # Safety
/// `async_obj` must be a valid `IVssAsync` COM pointer obtained from a VSS method
/// (e.g., `PrepareForBackup` or `DoSnapshotSet`).
fn wait_for_async(async_obj: ComPtr, operation: &str) -> Result<()> {
    if async_obj.is_null() {
        return Ok(());
    }

    // SAFETY: async_obj is a valid IVssAsync COM pointer. The vtable accessor reads
    // the first pointer-sized field which is the vtable pointer (COM convention).
    // Wait() and QueryStatus() are at known offsets in the IVssAsync vtable.
    unsafe {
        let vtbl = async_obj.async_vtbl();

        // Wait for completion (blocking)
        let hr = (vtbl.wait)(async_obj.as_ptr());
        if hr < 0 {
            return Err(Error::Message {
                message: format!(
                    "IVssAsync::Wait failed for `{operation}`: 0x{:08X}",
                    hr as u32
                ),
            });
        }

        // Check status
        let mut hr_result: i32 = 0;
        let hr = (vtbl.query_status)(async_obj.as_ptr(), &mut hr_result, ptr::null_mut());
        if hr < 0 || hr_result != VSS_S_ASYNC_FINISHED {
            return Err(Error::Message {
                message: format!(
                    "VSS async `{operation}` failed: status=0x{:08X}, result=0x{:08X}",
                    hr as u32, hr_result as u32
                ),
            });
        }
    }

    Ok(())
}

// ── Wide string helpers ────────────────────────────────────────────────────

fn wide_string(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

fn from_wide_ptr(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

fn guid_to_string(id: &VssId) -> String {
    id.to_guid_string()
}

fn parse_guid(s: &str) -> Result<VssId> {
    let s = s.trim().trim_start_matches('{').trim_end_matches('}');
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return Err(Error::InvalidArgument {
            message: format!("invalid GUID: `{s}`"),
        });
    }
    let data1 = u32::from_str_radix(parts[0], 16).map_err(|_| Error::InvalidArgument {
        message: format!("invalid GUID data1: `{}`", parts[0]),
    })?;
    let data2 = u16::from_str_radix(parts[1], 16).map_err(|_| Error::InvalidArgument {
        message: format!("invalid GUID data2: `{}`", parts[1]),
    })?;
    let data3 = u16::from_str_radix(parts[2], 16).map_err(|_| Error::InvalidArgument {
        message: format!("invalid GUID data3: `{}`", parts[2]),
    })?;
    let d4_full = format!("{}{}", parts[3], parts[4]);
    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&d4_full[i * 2..i * 2 + 2], 16).map_err(|_| {
            Error::InvalidArgument {
                message: format!("invalid GUID data4 byte {i}"),
            }
        })?;
    }
    Ok(VssId {
        data1,
        data2,
        data3,
        data4,
    })
}

// ── Volume path normalization ──────────────────────────────────────────────

fn normalize_volume_path(path: &str) -> String {
    if path.starts_with(r"\\?\Volume{") || path.starts_with(r"\\.\Volume{") {
        return path.to_string();
    }
    // Drive letter like "C:" → "C:\"
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        let trimmed = path.trim_end_matches('\\').trim_end_matches('/');
        return format!("{}\\", trimmed);
    }
    path.to_string()
}

// ── HRESULT helpers ────────────────────────────────────────────────────────

fn hr_ok(hr: i32, operation: &'static str) -> Result<()> {
    if hr >= 0 {
        Ok(())
    } else {
        Err(Error::Message {
            message: format!(
                "VSS COM error in `{operation}`: HRESULT 0x{:08X}",
                hr as u32
            ),
        })
    }
}

// ── Shared snapshot info builder ───────────────────────────────────────────

pub fn snapshot_info_from_device(
    snapshot_id: &str,
    source_volume: &str,
    device_path: &str,
) -> super::RawSnapshotSet {
    super::RawSnapshotSet {
        snapshot_set_id: snapshot_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
        device_path: device_path.to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Public COM API functions
// ═══════════════════════════════════════════════════════════════════════════

/// Initialize COM for VSS operations (called once).
pub fn initialize() -> Result<()> {
    ensure_com_initialized()
}

/// Create a VSS snapshot set following the C++ VssClient.cpp workflow.
///
/// Returns the snapshot ID and device path on success.
pub fn create_snapshot(volume_path: &str) -> Result<super::RawSnapshotSet> {
    ensure_com_initialized()?;

    let volume = normalize_volume_path(volume_path);
    info!(volume = %volume, "creating VSS snapshot via COM API");

    // Create IVssBackupComponents
    let bc = create_backup_components()?;

    // SAFETY: `bc` is a valid IVssBackupComponents COM pointer obtained from
    // CreateVssBackupComponentsInternal in vssapi.dll. All vtable method pointers
    // are called with the correct COM object pointer as the first argument.
    // Vtable indices match the Windows SDK IVssBackupComponents interface layout
    // (empirically verified — see TODO.md for vtable debugging notes).
    unsafe {
        let vtbl = bc.backup_components_vtbl();

        // InitializeForBackup() — NO args (matching C++ line 624)
        hr_ok(
            (vtbl.initialize_for_backup)(bc.as_ptr()),
            "InitializeForBackup",
        )?;

        // SetContext(VSS_CTX_APP_ROLLBACK) — matching C++ line 627
        hr_ok(
            (vtbl.set_context)(bc.as_ptr(), VSS_CTX_APP_ROLLBACK),
            "SetContext",
        )?;

        // SetBackupState(true, false, VSS_BT_FULL, false) — 4 args! matching C++ line 630
        hr_ok(
            (vtbl.set_backup_state)(bc.as_ptr(), true, false, VSS_BT_FULL, false),
            "SetBackupState",
        )?;

        // StartSnapshotSet
        let mut snapshot_set_id = VssId::ZERO;
        hr_ok(
            (vtbl.start_snapshot_set)(bc.as_ptr(), &mut snapshot_set_id),
            "StartSnapshotSet",
        )?;

        // AddToSnapshotSet
        let volume_wide = wide_string(&volume);
        let mut snapshot_id = VssId::ZERO;
        hr_ok(
            (vtbl.add_to_snapshot_set)(
                bc.as_ptr(),
                volume_wide.as_ptr(),
                VssId::ZERO,
                &mut snapshot_id,
            ),
            "AddToSnapshotSet",
        )?;

        info!(
            snapshot_set_id = %guid_to_string(&snapshot_set_id),
            snapshot_id = %guid_to_string(&snapshot_id),
            volume = %volume,
            "VSS snapshot set started"
        );

        // PrepareForBackup (async) — matching C++ PrepareForBackupSync
        {
            let mut async_obj: *mut c_void = ptr::null_mut();
            hr_ok(
                (vtbl.prepare_for_backup)(bc.as_ptr(), &mut async_obj),
                "PrepareForBackup",
            )?;
            if !async_obj.is_null() {
                wait_for_async(ComPtr::from_raw(async_obj), "PrepareForBackup")?;
            }
        }

        // DoSnapshotSet (async) — matching C++ DoSnapshotSetSync
        {
            let mut async_obj: *mut c_void = ptr::null_mut();
            hr_ok(
                (vtbl.do_snapshot_set)(bc.as_ptr(), &mut async_obj),
                "DoSnapshotSet",
            )?;
            if !async_obj.is_null() {
                wait_for_async(ComPtr::from_raw(async_obj), "DoSnapshotSet")?;
            }
        }

        // Get snapshot properties to obtain the device path
        let mut props = std::mem::zeroed::<VssSnapshotProp>();
        hr_ok(
            (vtbl.get_snapshot_properties)(bc.as_ptr(), snapshot_id, &mut props),
            "GetSnapshotProperties",
        )?;

        let device_path = from_wide_ptr(props.snapshot_device_object);
        vss_free_snapshot_properties(&mut props);

        info!(
            snapshot_id = %guid_to_string(&snapshot_id),
            device_path = %device_path,
            "VSS snapshot created"
        );

        Ok(super::RawSnapshotSet {
            snapshot_set_id: guid_to_string(&snapshot_set_id),
            snapshot_id: guid_to_string(&snapshot_id),
            device_path,
        })
    }
}

/// Delete a VSS snapshot by GUID using IVssCoordinator.
/// The VSS Coordinator CLSID IS registered on Windows, unlike CVssBackupComponents.
pub fn delete_snapshot(snapshot_id: &str) -> Result<()> {
    ensure_com_initialized()?;

    let coord = create_coordinator()?;
    let id = parse_guid(snapshot_id)?;

    info!(snapshot_id = %snapshot_id, "deleting VSS snapshot via IVssCoordinator");

    unsafe {
        let vtbl = coord.coordinator_vtbl();
        let mut deleted_count: i32 = 0;
        let mut non_deleted = VssId::ZERO;

        let hr = (vtbl.delete_snapshots)(
            coord.as_ptr(),
            id,
            VSS_OBJECT_SNAPSHOT,
            false,
            &mut deleted_count,
            &mut non_deleted,
        );
        debug!(
            "DeleteSnapshots returned 0x{:08X}, deleted={}",
            hr as u32, deleted_count
        );
        hr_ok(hr, "DeleteSnapshots")?;
    }

    info!(snapshot_id = %snapshot_id, "VSS snapshot deleted via COM");
    Ok(())
}

/// Create IVssCoordinator via CoCreateInstance.
/// CLSID_VSSCoordinator IS registered on Windows.
fn create_coordinator() -> Result<ComPtr> {
    // CLSID_VSSCoordinator: {E579AB5F-1CC4-44b4-BED9-DE0991FF0623}
    let clsid = VssId {
        data1: 0xE579AB5F,
        data2: 0x1CC4,
        data3: 0x44b4,
        data4: [0xBE, 0xD9, 0xDE, 0x09, 0x91, 0xFF, 0x06, 0x23],
    };
    // IID_IVssCoordinator: {DA9F41D4-1A5D-41d0-A614-6DFD78DF5D05}
    let iid = VssId {
        data1: 0xDA9F41D4,
        data2: 0x1A5D,
        data3: 0x41d0,
        data4: [0xA6, 0x14, 0x6D, 0xFD, 0x78, 0xDF, 0x5D, 0x05],
    };

    let raw = ole32_co_create_instance(&clsid, &iid)?;

    debug!("IVssCoordinator created at {:?}", raw);
    Ok(ComPtr::from_raw(raw))
}

// Raw ole32 CoCreateInstance - use windows_core::link! macro for deferred loading.
// This avoids name conflicts with the windows crate's typed wrapper.
fn ole32_co_create_instance(clsid: &VssId, iid: &VssId) -> Result<*mut c_void> {
    // Use the windows crate's internal CoCreateInstance with raw pointers
    let clsid_guid: &windows::core::GUID = unsafe { std::mem::transmute(clsid) };
    let iid_guid: &windows::core::GUID = unsafe { std::mem::transmute(iid) };
    let mut raw: *mut c_void = ptr::null_mut();

    // Call ole32!CoCreateInstance directly via raw FFI
    type CoCreateInstanceFn = unsafe extern "system" fn(
        *const VssId,
        *const c_void,
        u32,
        *const VssId,
        *mut *mut c_void,
    ) -> i32;

    // The windows crate already links ole32.dll, so we can use GetProcAddress
    // or just link it ourselves. Actually, let's use the extern declaration.
    // On x64 Windows, all stdcall functions use the same ABI as C.
    unsafe {
        unsafe extern "system" {
            fn CoCreateInstance(
                rclsid: *const VssId,
                punkouter: *const c_void,
                dwclscontext: u32,
                riid: *const VssId,
                ppv: *mut *mut c_void,
            ) -> i32;
        }
        let hr = CoCreateInstance(clsid, ptr::null(), 0x1F, iid, &mut raw);
        hr_ok(hr, "CoCreateInstance")?;
    }

    Ok(raw)
}

/// List all VSS snapshots matching the given volume, using IVssBackupComponents::Query.
pub fn list_snapshots(source: &VolumeRef, backend: &'static str) -> Result<Vec<SnapshotInfo>> {
    ensure_com_initialized()?;

    let volume_path = normalize_volume_path(&source.id);
    let mut snapshots = Vec::new();

    let bc = create_backup_components()?;

    unsafe {
        let vtbl = bc.backup_components_vtbl();

        // Query(GUID_NULL, VSS_OBJECT_NONE, VSS_OBJECT_SNAPSHOT, &pEnum)
        // matching C++ VssClient::QueryAllSnapshots line 455
        let mut enum_raw: *mut c_void = ptr::null_mut();
        let hr = (vtbl.query)(
            bc.as_ptr(),
            VssId::ZERO,         // IID_NULL
            VSS_OBJECT_NONE,     // eObjectType
            VSS_OBJECT_SNAPSHOT, // eReturnedObjectsType
            &mut enum_raw,
        );
        if hr < 0 || enum_raw.is_null() {
            info!(volume = %volume_path, "no VSS snapshots found");
            return Ok(snapshots);
        }

        let enum_obj = ComPtr::from_raw(enum_raw);
        let enum_vtbl = enum_obj.enum_vtbl();

        loop {
            let mut prop = std::mem::zeroed::<VssObjectProp>();
            let mut fetched: u32 = 0;
            let hr = (enum_vtbl.next)(enum_obj.as_ptr(), 1, &mut prop, &mut fetched);
            if hr != 0 || fetched == 0 {
                break;
            }

            if prop.object_type == VSS_OBJECT_SNAPSHOT {
                let orig_vol = from_wide_ptr(prop.snapshot.original_volume_name);
                if orig_vol
                    .trim_end_matches('\\')
                    .eq_ignore_ascii_case(volume_path.trim_end_matches('\\'))
                {
                    let device = from_wide_ptr(prop.snapshot.snapshot_device_object);
                    let exposed = from_wide_ptr(prop.snapshot.exposed_name);
                    let path_hint = if !exposed.is_empty() {
                        Some(std::path::PathBuf::from(&exposed))
                    } else if !device.is_empty() {
                        Some(std::path::PathBuf::from(&device))
                    } else {
                        None
                    };

                    snapshots.push(SnapshotInfo {
                        handle: SnapshotHandle {
                            id: guid_to_string(&prop.snapshot.snapshot_id),
                            source: Some(VolumeRef::new(&volume_path)),
                        },
                        backend,
                        path_hint,
                        read_only: true,
                    });
                }
            }

            // Free BSTRs
            free_bstr(prop.snapshot.original_volume_name);
            free_bstr(prop.snapshot.originating_machine);
            free_bstr(prop.snapshot.service_machine);
            free_bstr(prop.snapshot.snapshot_device_object);
            free_bstr(prop.snapshot.exposed_name);
            free_bstr(prop.snapshot.exposed_path);
        }
    }

    info!(volume = %volume_path, count = snapshots.len(), "VSS snapshots listed via COM");
    Ok(snapshots)
}

/// Get the device path for a VSS snapshot.
pub fn get_snapshot_device_path(snapshot_id: &str) -> Result<String> {
    ensure_com_initialized()?;

    let bc = create_backup_components()?;
    let id = parse_guid(snapshot_id)?;

    unsafe {
        let vtbl = bc.backup_components_vtbl();
        let mut props = std::mem::zeroed::<VssSnapshotProp>();
        let hr = (vtbl.get_snapshot_properties)(bc.as_ptr(), id, &mut props);
        if hr < 0 {
            return Err(Error::Message {
                message: format!("GetSnapshotProperties failed: 0x{:08X}", hr as u32),
            });
        }

        let device = from_wide_ptr(props.snapshot_device_object);
        let exposed = from_wide_ptr(props.exposed_name);
        vss_free_snapshot_properties(&mut props);

        if !exposed.is_empty() {
            Ok(exposed)
        } else if !device.is_empty() {
            Ok(device)
        } else {
            Err(Error::Message {
                message: "snapshot has no device path".to_string(),
            })
        }
    }
}

fn free_bstr(ptr: *const u16) {
    if !ptr.is_null() {
        unsafe { SysFreeString(ptr) };
    }
}

/// Dynamically load VssFreeSnapshotProperties from vssapi.dll.
fn vss_free_snapshot_properties(props: *mut VssSnapshotProp) {
    let name = wide_string("vssapi.dll");
    let handle = unsafe { LoadLibraryW(name.as_ptr()) };
    if handle.is_null() {
        return;
    }
    let proc_name = b"VssFreeSnapshotProperties\0";
    let proc = unsafe { GetProcAddress(handle, proc_name.as_ptr()) };
    if !proc.is_null() {
        let free_fn: unsafe extern "system" fn(*mut VssSnapshotProp) =
            unsafe { std::mem::transmute(proc) };
        unsafe { free_fn(props) };
    }
    unsafe { FreeLibrary(handle) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_guid_roundtrip() {
        let s = "{12345678-ABCD-EF01-1122-334455667788}";
        let id = parse_guid(s).unwrap();
        assert_eq!(id.to_guid_string(), s);
    }

    #[test]
    fn parse_guid_without_braces() {
        let s = "12345678-ABCD-EF01-1122-334455667788";
        let id = parse_guid(s).unwrap();
        assert_eq!(id.data1, 0x12345678);
        assert_eq!(id.data2, 0xABCD);
        assert_eq!(id.data3, 0xEF01);
    }

    #[test]
    fn parse_guid_rejects_invalid_format() {
        assert!(parse_guid("not-a-guid").is_err());
        assert!(parse_guid("{1234-5678}").is_err());
        assert!(parse_guid("").is_err());
    }

    #[test]
    fn normalize_volume_path_drive_letter() {
        assert_eq!(normalize_volume_path("C:"), "C:\\");
        assert_eq!(normalize_volume_path("C:\\"), "C:\\");
        assert_eq!(normalize_volume_path("D:"), "D:\\");
    }

    #[test]
    fn normalize_volume_path_guid_passthrough() {
        let guid = r"\\?\Volume{12345678-abcd-ef01-1122-334455667788}\";
        assert_eq!(normalize_volume_path(guid), guid);
    }

    #[test]
    fn wide_string_produces_null_terminated_utf16() {
        let w = wide_string("Hi");
        assert_eq!(w, vec![72, 105, 0]); // 'H', 'i', NUL
    }

    #[test]
    fn from_wide_ptr_null_returns_empty() {
        assert_eq!(from_wide_ptr(ptr::null()), "");
    }

    #[test]
    fn from_wide_ptr_reads_utf16() {
        // "AB" in UTF-16: 0x0041, 0x0042
        let data: Vec<u16> = vec![0x0041, 0x0042, 0x0000];
        assert_eq!(from_wide_ptr(data.as_ptr()), "AB");
    }

    #[test]
    fn vss_id_zero_is_all_zeros() {
        let id = VssId::ZERO;
        assert_eq!(id.data1, 0);
        assert_eq!(id.data2, 0);
        assert_eq!(id.data3, 0);
        assert_eq!(id.data4, [0; 8]);
    }
}
