# Provider Status

## Linux

### Btrfs
Status: implemented.

Implemented:

- snapshot create/list/delete via `btrfs subvolume snapshot`, `btrfs subvolume list -s`, `btrfs subvolume delete`
- send-based backup to a stream file via `btrfs send`
- receive-based restore from a stream file via `btrfs receive`
- incremental send with parent snapshot (`btrfs send -p`)
- temporary snapshot policy for backup planning
- snapshot path derivation under `.vb-snapshots/` hidden directory
- capability reporting and CLI selection
- backup CLI control over snapshot kind/label/read-only policy

Not implemented:

- mount/unmount flows (capabilities removed until implemented)
- privileged round-trip integration tests against a real Btrfs filesystem

### LVM
Status: implemented.

Implemented:

- logical volume path parsing (`/dev/<vg>/<lv>`)
- snapshot create/list/delete through the LVM CLI (`lvcreate --snapshot`, `lvs`, `lvremove`)
- read-only snapshot permission adjustment (`lvchange --permission r`)
- image-file backup through block-level copy (`copy_blocks`, default 4 MiB)
- image-file restore through block-level copy (requires `--force`)
- temporary snapshot policy for backup planning
- explicit `--force` guard for destructive restore
- capability reporting and CLI selection

Not implemented:

- mount/unmount flows (capabilities removed until implemented)
- incremental/differential export semantics

### ZFS
Status: implemented.

Implemented:

- dataset reference parsing (dataset name or mount path)
- snapshot create/list/delete through the ZFS CLI (`zfs snapshot`, `zfs list -t snapshot`, `zfs destroy`)
- file-based backup through `zfs send` (with `-i` for incremental)
- file-based restore through `zfs receive` (with `-F` for force)
- parent snapshot support in send planning
- explicit snapshot source requirement for backup (or temporary snapshot policy)
- rejection of mount-path destinations for receive
- capability reporting and CLI selection

Not implemented:

- mount/unmount flows (capabilities removed until implemented)
- automatic snapshot creation for backup without explicit snapshot policy

## Windows
Status: implemented (feature-gated: `windows-vss`).

The Windows backend uses a dual-path strategy:

- **CLI path (primary)**: `wmic shadowcopy` for snapshot creation and device path retrieval, `vssadmin` for listing and deletion. Works on all Windows editions (Home, Pro, Server).
- **COM API (fallback)**: Native `IVssBackupComponents` and `IVssCoordinator` via raw vtable FFI. Used primarily for snapshot deletion when CLI fails.

Known limitations:

- COM snapshot creation (`InitializeForBackup`) returns `VSS_E_BAD_STATE` on some Windows editions due to vtable layout mismatches. See `TODO.md` for detailed analysis.
- The CLI path handles all snapshot creation reliably.
- Mount/unmount flows are not implemented (capabilities removed).

## macOS And Generic Unix
Status: stubbed.

Current modules expose backend identities and capability sets, but no operational snapshot logic yet. All trait methods return `UnsupportedOperation`.
