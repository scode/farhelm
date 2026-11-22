# Trash delete drops the removal report and skips post-delete rechecks on unordered refresh races

Reviewed commit: 21c2cdfb1e650bedba1a874a9082e7a8cb0231ea

## TLDR

When deleting archived checkouts from Trash, if a Trash refresh fails before the delete reply arrives, the Trash icon
stops showing that the host is freeing space instead of retaining the removal status as specified. Also, if a refresh
triggered while a delete is in flight runs before the delete on the host and the delete later settles without advancing
the Trash epoch (such as a refusal or non-timeout failure), Trash skips the post-delete recheck.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/trash.rs:691`.

In `crates/gui/farhelm-gui/src/trash.rs:691-710` (and `crates/gui/farhelm-gui/src/trash/listing.rs:296-315`), switching
both Trash reads (`operations::send_unordered` at `trash.rs:309`) and `Work::TrashDelete` (`operations.rs:259`) to
`Lane::Unordered` allows a refresh issued after the delete started (`entry.serial != read_before`, so
`read_since == true`) to complete before `TrashButton::deleted` runs (the commit's own test
`a_late_delete_reply_keeps_the_newer_trash_listing` in `crates/gui/farhelm-gui/src/trash/dialog/tests.rs:335-384`
exercises this exact ordering via the pushed `trash_epoch`, and `TrashButton::open` -> `measure()` at `trash.rs:528` or
`TrashButton::recheck` at `trash.rs:383-393` can also issue a read while a delete is in flight).

Two defects follow from `TrashButton::deleted` treating `read_since && !entry.loading` as equivalent to 'a post-delete
refresh already succeeded':

1. **Failed post-delete read clears/drops the removal report**: Suppose a capable host (`reports_retired == true`,
   `entry.removing == false`) accepts a delete (`removed: ["a"]`), advances `trash_epoch`, and pushes `Hosts` before the
   delete reply settles, or another read issued after the delete starts fails before `deleted()` runs.
   `TrashButton::listed` (`trash.rs:360-371`) sets `entry.loading = false`, replaces `entry.listing` with
   `Some(Err(words))`, leaves `entry.removing == false`, and schedules no recheck. When `TrashButton::deleted` then runs
   with `Ok(deleted)`, `(!read_since || entry.loading)` at `trash.rs:697` is `false`, so `listing::accepted_cleanup` is
   skipped, and `!read_since && !timed_out` at `trash.rs:708` is also `false`. Furthermore, even if `accepted_cleanup`
   were called, `trash/listing.rs:297` immediately returns on
   `let Some(Ok(listing)) = state.listing.as_mut() else { return; };` because `HostTrash` does not retain
   `reports_retired` across a failed read. As a result, `entry.removing` stays `false` and the Trash button stops
   showing the host as freeing space. This contradicts both `SPEC.md` ('Managed checkouts': 'A failed read retains the
   last removal report rather than implying that removal finished') and `SPEC_impl.md:3918-3920` (modified by this same
   commit: 'A capable acceptance marks the host as removing, including for an accepted diagnostic-only ID, so a failed
   read right after can retain that report. If a read issued after the delete has already succeeded and no refresh is
   outstanding, that listing supplies the removal state.').
2. **Unordered read racing ahead of the delete suppresses post-delete recheck**: Because both
   `POST /api/checkout-trash/list` and `POST /api/checkout-trash/delete` run concurrently in `ui_channel::run`'s
   `FuturesUnordered` (`crates/farhelm-helm/src/ui_channel.rs:1039`) and compete for
   `state.trash_requests.lock(req.host)` (`crates/farhelm-helm/src/sessions.rs:636,704`), a read started by
   `open()`/`measure()` or `recheck()` while a delete is in flight can acquire the lock and return the pre-delete
   listing before the delete executes. If the delete then finishes without advancing `trash_epoch` (e.g.,
   `deleted.removed.is_empty()` with per-ID issues at `sessions.rs:718`, or a non-504 error), `read_since` is still
   `true` from that pre-delete read, so `trash.rs:708` skips `self.read(target.host, cx)` instead of rechecking the
   listing after the delete settles (and if `Hosts` is temporarily `Unavailable` during a successful delete's push,
   `sync()` does not start a new read while `read_since && !entry.loading` also suppresses `accepted_cleanup` on the
   stale pre-delete listing).

Fix direction: Track `reports_retired` (or the host's last known capability) on `HostTrash` across failed reads so a
capable delete acceptance can still set `entry.removing = true` when `entry.listing` is `Some(Err(_))`; only suppress
`accepted_cleanup` when a read issued after the delete **succeeded** (not failed) and is known not to predate the
delete's effect (e.g., triggered by an epoch >= the delete's or issued after the delete replied); and always trigger a
post-delete read on non-504 settlement unless an epoch-triggered read after the delete is already in flight or has
already succeeded.

Reviewer metadata (g41-desktop-bypass-slow-hosts/l1): suggested bucket: other (could not be highest: no security issue
or loss of user data/credentials/processes/work; Trash delete was already user-authorized and a failed listing keeps
delete controls disabled); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g41-desktop-bypass-slow-hosts/l1`.
