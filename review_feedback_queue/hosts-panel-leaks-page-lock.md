# Host actions can strand the page-wide operation lock

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the browser has to ask for the access token again while a host action is running (retry, adopt, forget, retarget,
alias, or the YOLO setting), then after the user signs back in every button in the app silently refuses to work until
the page is reloaded.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F10 / COR-HOSTS-LOCK`, tagged **definite**. Anchor and title: `crates/farhelm-ui/src/hosts.rs:879` — host
actions release the page-wide lock by hand, so a browser re-login during one leaves the page inert.

The web UI has one page-wide operation lock, owned by the top-level authenticated page component (`AppBody`, created at
`crates/farhelm-ui/src/lib.rs:1211`). While it is held, creating a session, sidebar row actions, host actions and even
opening a row refuse to start, so two operations never run at once. The lock can be taken two ways
(`crates/farhelm-ui/src/ops.rs`): a bare `claim()` that the caller must later `release()` by hand, or `claim_guard()`,
which returns a guard that releases the lock when dropped. The guard's docs say to move it into the spawned task so that
if the task is dropped because its component unmounted, the lock is released anyway. The guard is already used elsewhere
(`hosts.rs:3020`, `provisioning.rs:1949`, `:2192`).

The hosts panel's shared request runner, which carries retry, adopt, forget, retarget, alias and the YOLO setting, uses
the bare form: it calls `ops.claim()` at `crates/farhelm-ui/src/hosts.rs:879`, spawns the request at `:890`, and calls
`ops.release()` at the end of that task at `:908`. In the browser, if any request gets a 401 (the access token was
rotated), the page raises the token prompt, and `lib.rs:1259` replaces the entire authenticated tree with that prompt.
This unmounts the hosts panel and drops its task before the `release()` line runs. `AppBody`, which owns the lock, stays
mounted, so the lock stays held. After the user pastes a new token, every lock-guarded control silently refuses, with
nothing on screen to explain why, until the page is reloaded.

An ordinary re-login thus leaves the page dead. The fix is small: take
`let Some(claim) = ops.claim_guard() else { return false; };`, move the guard into the spawned task, and drop it where
`release()` is today.

## Additional detail merged from a second review (de774a1ee8815ce833da77deac593a55d82f7be3)

A separate review found the same flaw independently (recorded first as an extension of
`session-view-leaks-page-lock.md`, then folded in here). It adds:

- The precise trigger: the action's own 401 releases the lock correctly, because nothing awaits between `Err` and
  `release()`. The lock leaks only when a different request (the event feed, a poll, or a detail read) gets the 401
  first and flips the page to the token prompt, which unmounts the subtree holding the spawned task.
- The window is widest for remove: the helm's `remove_host_owned` (`crates/farhelm-helm/src/hosts.rs:781-803`) waits for
  the host write lock and for `stop_actor` before replying.
- After the token exchange the subtree remounts and receives the same `page_ops` (`crates/farhelm-ui/src/lib.rs:1364`,
  `1403`), still held.
- Trigger background: SPEC.md (Security, around line 1340, and the decision around line 1711) says rotating the token
  invalidates every device credential for new requests. The trigger is rare, but the stuck page lasts until reload, so
  FILTER.md's rare-glitch filter does not apply.
- A browser verification recipe: hold a `DELETE /api/hosts/{id}` open, make another request return 401, submit a valid
  token, then try to create a session; it is refused and the create button stays disabled.
- The same bare-claim pattern in `profiles.rs:1437` and `1544` is `profile-popup-leaks-page-lock.md`.
- At main 1ec60cc (after 6d04159 moved host settings into a dialog) the shared `run` wrapper still claims at
  `crates/farhelm-ui/src/hosts.rs:879` and releases by hand at :908, and the settings dialog's writes go through it.
