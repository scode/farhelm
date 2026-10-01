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
