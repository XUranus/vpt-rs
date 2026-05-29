# TODO

## Near Term
- Design the generic API additions needed for incremental backup streams and parent snapshot references.
- Add demo CLI coverage for richer backup and restore options as provider support expands.

## Btrfs Follow-Up
- Add privileged integration tests for a real Btrfs environment.
- Validate `btrfs subvolume snapshot`, `btrfs send`, and `btrfs receive` in an end-to-end round-trip test.
- Add incremental `btrfs send -p <parent>` support once the shared plan types can express parent snapshots.
- Add mount-oriented workflows for browsing or exporting snapshot contents safely.

## Windows VSS — COM API Debug Notes

### Current Status (2026-05-29)

**Working (CLI fallback):**
- `wmic shadowcopy call create` — snapshot creation ✅
- `wmic shadowcopy where ID='{sid}' get DeviceObject` — device path ✅
- `wmic shadowcopy where ID='{sid}' delete` — deletion ✅
- `vssadmin list shadows` — listing with locale-independent parsing ✅
- `IVssCoordinator::DeleteSnapshots` at vtable[11] — COM deletion ✅
- `CoInitializeEx(COINIT_MULTITHREADED)` + `CoInitializeSecurity(...)` ✅

**Broken (COM snapshot creation):**
- `InitializeForBackup` returns VSS_E_BAD_STATE after successful call
- All subsequent methods (`SetContext`, `SetBackupState`, `StartSnapshotSet`) return VSS_E_BAD_STATE

### Test Environment

- Windows 10 Home China, build 19045 (21H2)
- vssapi.dll: 1,674,240 bytes, 2024-05-17
- Rust 1.95.0 (edition 2024), `windows` crate v0.62
- VSS service running

### Problem 1: `CreateVssBackupComponents` not exported

Only `CreateVssBackupComponentsInternal` is exported from vssapi.dll. The C++ code
uses `CreateVssBackupComponents` which is an inline wrapper in `vsbackup.h` that
calls `CreateVssBackupComponentsInternal`. We load it dynamically via `GetProcAddress`.

### Problem 2: vtable layout mismatch

The vtable returned by `CreateVssBackupComponentsInternal` does NOT match the
Windows SDK header layout. Empirically tested vtable (base `0x7FFB45BC9AB0`):

| Index | Function (assumed)       | Result              |
|-------|--------------------------|---------------------|
| 0-2   | IUnknown                 | N/A                 |
| 3     | GetWriterMetadataCount   | crash (write bad ptr) |
| 4     | GetWriterMetadata        | VSS_E_BAD_STATE     |
| 5     | FreeWriterMetadata       | S_OK                |
| **12**| **InitializeForBackup**  | **S_OK** ✅          |
| 13    | Unknown                  | crash (read bad ptr)|
| 15    | Unknown                  | VSS_E_BAD_STATE     |
| 18    | Unknown                  | VSS_E_BAD_STATE     |
| 20    | Unknown (was target)     | E_INVALIDARG        |

**InitializeForBackup is at index 12, not 20** (offset -8 from SDK header).

### Problem 3: Post-init methods all fail

After `InitializeForBackup` at [12] returns S_OK:
- [15] 4-arg: VSS_E_BAD_STATE
- [18] 2-arg: VSS_E_BAD_STATE
- [20] 4-arg: VSS_E_BAD_STATE
- [8] 4-arg: VSS_E_BAD_STATE

The object is in a bad state even after successful initialization.

### Problem 4: IVssCoordinator vtable also shifted

`IVssCoordinator` CLSID IS registered (unlike `CVssBackupComponents`).
`CoCreateInstance` succeeds. Vtable mapping:

| Index | Identity               | Result              |
|-------|------------------------|---------------------|
| 3     | SetContext             | S_OK                |
| 4     | StartSnapshotSet       | S_OK                |
| 5     | AddToSnapshotSet       | S_OK                |
| 6     | DoSnapshotSet          | CRASH (pure virtual)|
| 11    | DeleteSnapshots        | VSS_E_OBJECT_NOT_FOUND (works) |

Coordinator can create snapshot sets but `DoSnapshotSet` crashes (pure virtual call).

### Problem 5: CVssBackupComponents CLSID not registered

`{66849CDC-2C91-4B09-8B4C-1B10A1B7E08D}` is NOT in the registry.
`CoCreateInstance` with this CLSID returns `REGDB_E_CLASSNOTREG`.
Must use `CreateVssBackupComponentsInternal` (dynamic load).

### Hypotheses

1. **DLL version mismatch** — vssapi.dll on this system may have a different vtable
   layout than SDK headers target. Test on Windows 11 / Server 2019 / Server 2022.
2. **VSS subsystem misconfiguration** — writers may be in error state. Check
   `vssadmin list writers`. Try after clean reboot.
3. **Extended interface** — `CreateVssBackupComponentsInternal` may return
   `IVssBackupComponentsEx4` whose vtable has extra methods inserted between
   base methods, shifting all indices.
4. **COM apartment model** — try `COINIT_APARTMENTTHREADED` (0x2) instead of
   `COINIT_MULTITHREADED` (0x0).
5. **Calling convention** — Python ctypes `CFUNCTYPE` (cdecl) vs C# `stdcall`
   produce different error codes for same function pointer. x64 should have
   one calling convention, but register state may differ.

### Future Analysis Paths

**Path A**: Compile C++ reference (`Win32VSSWrapper/Demo.cpp`) on the target system
with MSVC, dump vtable addresses, compare with Rust/Python. Requires MSVC or MinGW.

**Path B**: Define `IVssBackupComponents` using `windows_core::imp::define_interface!`
macro to let the `windows` crate handle vtable generation correctly. Would need
the full interface definition (26 methods) and correct GUIDs.

**Path C**: Use WMI `Win32_ShadowCopy` class for ALL operations (currently only used
for creation). `wmic` already uses this. Could use PowerShell `Get-CimInstance`
or direct WMI COM calls.

**Path D**: Query for `IVssBackupComponentsEx` via `QueryInterface` with
IID `{963f03ad-9e4c-4a74-910b-082462c0a035}`. The extended interface might have
correct vtable.

**Path E**: Check VSS writer status (`vssadmin list writers`). Failed writers could
cause VSS_E_BAD_STATE.

### Code Locations

| File | Purpose |
|------|---------|
| `src/platform/windows/vss/ffi/com.rs` | COM vtable definitions, raw calls |
| `src/platform/windows/vss/ffi/cli.rs` | wmic/vssadmin CLI wrappers |
| `src/platform/windows/vss/ffi.rs` | Module root, routing |
| `src/platform/windows/vss.rs` | VssSnapshotProvider, SnapshotProvider impl |
| `src/platform/windows.rs` | WindowsBackend (all 4 traits) |
| `Win32VSSWrapper/VssClient.cpp` | Working C++ reference implementation |
| `tests/test_vss.py` | VHD-based end-to-end test |

## Other Providers
- Add privileged integration tests for the Linux ZFS provider.
- Add privileged integration tests for the Linux LVM provider in CI or an environment-gated harness.
- Add incremental `zfs send -i/-I` support once the shared plan model can express parent/base snapshots.
- Decide whether backup flows should auto-create temporary ZFS snapshots or keep requiring explicit snapshot identifiers.
- Design macOS APFS snapshot support behind the shared snapshot traits.
