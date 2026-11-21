# The ensure-hosts repair message omits the cost of rebuilding a staged host entry

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The repair message omits that removing and re-adding a staged host forgets its saved organization and settings.

## Details

The parent and fix differ only in the warning text at crates/farhelm-helm/src/ensure.rs:200–207. The earlier advice
pointed to Set up. The new advice is to correct the file, remove the entry if its saved paths are invalid, then restart.
The patch changes neither removal nor registration or existing data loss semantics. At the endpoint, ensure.rs:113–126
still supplies only the destination and two paths for a missing row, per SPEC_impl.md:5585–5591.
crates/farhelm-helm/src/store.rs:8329–8334 and :8394–8397 assign a fresh host ID on re-registration and drop the old
row. Crates/farhelm-helm/src/store/session_groups.rs:23–29 and :110–114 cascade memberships and prune empty groups. The
SSH Remove prompt at crates/gui/farhelm-gui/src/hosts/remove.rs:113–117 and :239–248 describes what survives remotely.
Warn about the staged registry reset before recommending removal, or repair the paths without replacing the row.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `host/l8`.
