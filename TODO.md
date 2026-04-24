# TODO

## Near Term
- Implement the Linux LVM provider with real snapshot create/list/delete flows.
- Design the generic API additions needed for incremental backup streams and parent snapshot references.
- Add demo CLI coverage for richer backup and restore options as provider support expands.

## Btrfs Follow-Up
- Add privileged integration tests for a real Btrfs environment.
- Validate `btrfs subvolume snapshot`, `btrfs send`, and `btrfs receive` in an end-to-end round-trip test.
- Add incremental `btrfs send -p <parent>` support once the shared plan types can express parent snapshots.
- Add mount-oriented workflows for browsing or exporting snapshot contents safely.

## Windows VSS
- Implement real VSS COM bindings behind `windows-vss`.
- Map VSS snapshot metadata into the shared snapshot model.
- Add snapshot enumeration and deletion through the requestor/session layer.

## Other Providers
- Implement the Linux ZFS provider with snapshot and send/receive support.
- Design macOS APFS snapshot support behind the shared snapshot traits.
