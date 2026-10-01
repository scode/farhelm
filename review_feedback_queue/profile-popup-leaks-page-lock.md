# Saving or deleting a profile can strand the page-wide operation lock

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the browser has to ask for the access token again while a profile is being saved or deleted, then after the user
signs back in every button in the app silently refuses to work until the page is reloaded.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F11 / COR-PROFILE-LOCK`, tagged **definite**. Anchor and title: `crates/farhelm-ui/src/profiles.rs:1437` —
saving or deleting a profile releases the page lock by hand, with the same re-login failure.

This is the same failure as F10, at a second site. The web and desktop UI keep one page-wide "an operation is in flight"
lock. While it is held, almost every action in the app refuses to start: creating a session, the sidebar's row actions,
host actions, and even opening a session row. The UI code offers two ways to take it. One is a bare claim that the
caller must later release by hand. The other is a guard object that releases the lock automatically when it is dropped.
The lock's own documentation (`crates/farhelm-ui/src/ops.rs`, on `OpGuard`) warns that a hand-written release at the end
of a background task "is not a release guarantee". A UI component's background tasks are cancelled when the component
disappears, and a cancelled task never reaches its last line.

In the profiles popup (`crates/farhelm-ui/src/profiles.rs`), both the save handler (claim at line 1437, releases at 1474
and 1532 among others) and the delete-confirm handler (claim at 1544, release at 1587) use the bare claim. They start
the request in a task tied to the popup and release the lock on that task's last line. The popup's own controls cannot
dismiss it mid-request, because those controls refuse while the lock is held. So the trigger that remains is
authentication. In the browser, a 401 from the helm makes the app swap the whole signed-in part of the page for the
token prompt (`crates/farhelm-ui/src/lib.rs` around line 1259). That unmounts the popup and cancels its task before it
releases the lock. The lock itself is owned higher up, by the top-level app component, which stays mounted. So after the
user pastes a fresh token, the lock is still held, and every lock-gated control silently refuses until the page is
reloaded.

The fix is to switch both handlers to the guard form (`claim_guard()`) and move the guard into the spawned task, so
cancelling the task releases the lock. In the save handler, take the guard only after the local validation checks that
currently return early (built-in profile, unparseable draft), or let those early returns simply drop it.
