# Provider Status

## Linux

### Btrfs
Status: partially implemented.

Implemented:

- snapshot create/list/delete
- send-based backup to a stream file
- receive-based restore from a stream file
- snapshot mount/unmount through bind mount + remount
- capability reporting and CLI selection
- temporary snapshot policy for backup planning
- parent snapshot support in send planning
- backup CLI control over snapshot kind/label/read-only policy

Not implemented:

- privileged round-trip integration tests against a real Btrfs filesystem

### LVM
Status: partially implemented.

Implemented:

- backend registration
- capability reporting
- CLI selection path
- logical volume path parsing
- snapshot create/list/delete through the LVM CLI
- read-only snapshot permission adjustment after creation
- image-file backup through `dd`
- image-file restore through `dd`
- temporary snapshot policy for backup planning
- explicit `--force` guard for destructive restore
- snapshot device mount/unmount through `mount` and `umount`

Not implemented:

- incremental/differential export semantics

### ZFS
Status: partially implemented.

Implemented:

- backend registration
- capability reporting
- CLI selection path
- dataset reference parsing
- snapshot create/list/delete through the ZFS CLI
- snapshot enumeration parsing from `zfs list -t snapshot`
- file-based backup through `zfs send`
- file-based restore through `zfs receive`
- parent snapshot support in send planning
- backup CLI control over explicit snapshot sources and parent snapshots
- read-only snapshot browsing through `.zfs/snapshot` bind mounts

Not implemented:

- dataset-oriented restore planning
- automatic snapshot creation for backup
- writable snapshot browsing flows
- privileged integration tests against a real ZFS environment

## Windows
Status: architecture prepared, implementation not started.

The project includes a dedicated VSS module tree for:

- request validation
- requestor/session separation
- a feature-gated FFI seam for future COM bindings

The real VSS COM implementation remains TODO.

## macOS And Generic Unix
Status: stubbed.

Current modules expose backend identities and capability sets, but no operational snapshot logic yet.
