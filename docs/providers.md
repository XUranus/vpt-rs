# Provider Status

## Linux

### Btrfs
Status: partially implemented.

Implemented:

- snapshot create/list/delete
- send-based backup to a stream file
- receive-based restore from a stream file
- capability reporting and CLI selection

Not implemented:

- incremental send with parent snapshots
- mount/unmount flows
- privileged round-trip integration tests against a real Btrfs filesystem

### LVM
Status: scaffolded only.

Current state:

- backend registration
- capability reporting
- CLI selection path

Next expected work:

- snapshot creation/deletion via `lvcreate`/`lvremove`
- snapshot metadata discovery
- restore/copy semantics for logical volumes

### ZFS
Status: scaffolded only.

Current state:

- backend registration
- capability reporting
- CLI selection path

Next expected work:

- snapshot lifecycle through `zfs snapshot` and `zfs destroy`
- send/receive integration
- dataset-oriented restore planning

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
