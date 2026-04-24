# TODO

## Near Term
- Add privileged integration tests for the Linux LVM provider.
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
- Add privileged integration tests for the Linux ZFS provider.
- Add incremental `zfs send -i/-I` support once the shared plan model can express parent/base snapshots.
- Decide whether backup flows should auto-create temporary ZFS snapshots or keep requiring explicit snapshot identifiers.
- Design macOS APFS snapshot support behind the shared snapshot traits.
