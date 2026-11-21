# native account lookup before a process-wide PATH write

Reviewed commit: 23ae2b9850c539d6d90284dd48322a98af7d78de

## TLDR

A supported account backend could leave an environment-reading thread active during the desktop PATH write. That backend
behavior remains unverified.

## Details

**other**. Unverified after investigation.

Unverified after investigation. Lib.rs:165-206 establishes the main-thread assumption and calls the direct passwd lookup
before set_var. Launch.rs:352-389 and :517-548 establish the host NSS boundary and the direct libc call. The pinned
application sources show synchronous tracing setup at lib.rs:469-477, no explicitly spawned Rust thread on the success
path, and a reaper restricted to timeout at login_path.rs:96-109, which skips the mutation. Rust std::env::set_var's
Unix safety contract requires that no other thread can concurrently read or write the environment through native APIs.
The local source search did not establish any stock supported passwd backend that leaves such a thread alive. The exact
missing premise is the threading and environment-read behavior of a supported native lookup implementation after
getpwuid_r returns. Source for that implementation, including any loaded account-service module, could settle it.
SPEC_impl.md:7486-7499 names both the native lookup and the single-threaded write. Estimated bucket: high only if the
supported startup crash route is established. No security or data-loss route established. Possible_cover: none
identified. Subsequent scoped changes retain the order, through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f10`.
