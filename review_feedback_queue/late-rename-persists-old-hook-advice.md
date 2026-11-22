# A late Rename can put a previous launch’s hook advice back in the cache

Reviewed commit: 199f513604dd6204f8a107e4314bcf8f9280167e

## TLDR

A late Rename reply can save the previous run’s hook advice in the helm cache until an authoritative refresh corrects
it.

## Details

**other.** Source inspection.

199f513604dd6204f8a107e4314bcf8f9280167e crates/farhelm-helm/src/sessions.rs:3588-3602,3636-3648;
crates/farhelm-helm/src/session_cache.rs:162-192; crates/farhelm-helm/src/store.rs:8997-9032;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/farhelm-helm/src/approvals.rs:552-556,890-897;
crates/farhelm-helm/src/agent_requests.rs:551-603; crates/farhelm-supervisor/src/service/handlers.rs:525-535,2347-2373;
crates/farhelm-supervisor/src/service/core.rs:3226-3252,11989-11997,13756-13786,14758-14769;
crates/farhelm-supervisor/src/service/listing.rs:108-139; crates/farhelm-supervisor/src/service/status.rs:339-350;
crates/farhelm-helm/src/manager.rs:2542-2554,2642-2650,5517-5543,5732-5822;
crates/farhelm-helm/src/store.rs:8830-8884,8914-8919; crates/farhelm-helm/src/ui_channel.rs:806-827,1059-1068;
crates/gui/farhelm-gui/src/session_pane/hook_band.rs:23-33,124-136; SPEC_impl.md:4525-4528

Rename can return the older launch's hook result after Restart has published the newer result. The helm commits the
older projection to its `session_cache`, which also supplies offline session rows. A successful authoritative refresh
after the stale commit corrects the cache. The setup switch and supervisor result live separately. Carry generation
ordering through the cache merge or restrict Rename to the title fields it changed. See
`sessions.rs:3588–3602,3636–3648`, `session_cache.rs:162–192` and `store.rs:8997–9032,9079–9094` at `199f51360`, plus
the pinned `manager.rs:2542–2554,2642–2650,5776–5822`.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-helm/HH-12`.
