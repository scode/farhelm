# Hook-log truncation can erase a concurrent line

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When several agent hooks in one session fire together, the diagnostic line explaining why resume capture failed can
silently vanish from the hook log.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F18 / COR-HOOK-LOG-TRUNCATION-RACE`, tagged **possible**. Anchors and title: `crates/farhelm/src/hook.rs:916`,
`crates/farhelm/src/hook.rs:918` — Hook-log truncation can erase a concurrent hook's just-written line

`farhelm internal hook` is a short-lived helper that agent harnesses (Claude, Codex, Grok, …) run as a callback to tell
Farhelm which conversation a session is using. It must print nothing, so its only diagnostic is a per-session **hook
log**, `<state_dir>/hook-log/<session-id>.log`, to which each run appends one line. `append_log` keeps the file bounded:
if it is over 64 KiB it truncates it with `File::create(path)` (L916–918) and then reopens with `O_APPEND` to write.
Nothing makes the size check and the truncation atomic with respect to other hooks.

When several hooks for the same session run at once (the module docs expect this, since several agents can run in one
session), hook B can append its line between hook A's size check and A's truncation, and A then erases it. The doc
comment promises the worst case is "whole lines out of order" and that "counting lines counts runs", which is false in
this case.

Since this log is the only place a hook failure is ever visible, losing the one line that explains a failed resume
capture defeats its purpose. Suggested change: rename an oversized file aside (for example to `<id>.log.old`) instead of
truncating it in place, or hold a short `flock` around check and truncation; failing that, correct the doc comment.

Restater note: the race needs a log already over 64 KiB (on the order of a thousand hook runs in one session) and two
hooks in that session firing within microseconds of each other, so it is rare.
