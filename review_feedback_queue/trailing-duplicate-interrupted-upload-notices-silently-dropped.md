# A second interrupted upload with the same session and file name is silently dropped

Reviewed commit: e3d03d5651e9f0b417514e145ce847a1e45fffe8

## TLDR

If an upload is interrupted when you leave a session and you do not dismiss its banner yet, returning to that session
and having another upload of the same file — or a different pasted screenshot — interrupted produces no new notice and
does not even scroll the existing banner into view, so the second failure disappears silently.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/notices.rs:93-99`.

Bucket note: `g20/f1` suggested `high` and `g19/f2` suggested `other`; placed in `high` because distinct pasted
screenshots reuse `pasted-1.png` and silently violate SPEC.md's upload-failure visibility guarantee.

`InterruptedUploads` (`crates/gui/farhelm-gui/src/notices.rs:93-99`) derives `PartialEq, Eq, Hash` solely over
`(session, title, files, sent)` with no unique occurrence id, and `notices::post`
(`crates/gui/farhelm-gui/src/notices.rs:177-186`) deduplicates via `if !notices.posted.contains(&notice)` without
appending, reordering, or calling `cx.notify()`. Because `Attachments::new`
(`crates/gui/farhelm-gui/src/terminal/attachments.rs:563`) resets `pasted: 0` on every new terminal, every newly opened
terminal for the same session names its first pasted image `pasted-1.<ext>` (`attachments.rs:612-617`). Thus:

1. Pasting a screenshot in session S, leaving S while it uploads (posting `(S, title, ["pasted-1.png"], sent)`), then
   later returning to S without dismissing the banner, pasting a completely different screenshot (`pasted-1.png`), and
   leaving S again builds an equal `Notice::InterruptedUpload` that `notices::post` silently ignores.
2. Retrying the same file name (e.g., `report.pdf`) in session S and leaving again while the earlier notice is still
   undismissed (for instance, scrolled out of view in a multi-banner stack) is likewise silently dropped, and `MainView`
   never scrolls to reveal it (`main_view.rs:188-190` only triggers `reveal_latest` when `posted.last()` was not in
   `drawn_banners`). This contradicts SPEC.md ("Attachments", lines 3641-3642): "Upload failures must be visible; an
   attachment must never disappear silently. An upload cut off by leaving its session is reported in a notice naming the
   file and the session, when its title is available, that stays until dismissed." Also note that
   `InterruptedUploads::key()` (`notices.rs:118-122`) hashes the struct fields for GPUI element IDs
   (`notice-interrupted-upload`, `notice-dismiss-interrupted-upload-{}`), so giving each `InterruptedUploads` instance a
   monotonic `u64` id (like `line_id()` in `attachments.rs:330-337`) would simultaneously prevent silent deduplication,
   keep each notice's GPUI element IDs unique, and avoid hash-collision risk in `key()`.

Reviewer metadata (g20-uploads-notices-main-view/f1): suggested bucket: high (could be in `highest`: no — the local file
is not deleted and clipboard screenshots are excluded from `highest` by Priority buckets, though the silent failure
violates the attachment visibility guarantee); confidence: confirmed; possible cover: none.

Additional source record at `e3d03d5651e9f0b417514e145ce847a1e45fffe8`,
`crates/gui/farhelm-gui/src/terminal/attachments.rs:429-441` (g19-uploads-terminal-attachments/f2):

Each `Attachments` instance starts its pasted-image counter at `pasted: 0`
(`crates/gui/farhelm-gui/src/terminal/attachments.rs:563, 612-617`), so every newly mounted terminal in a session names
its first pasted screenshot `pasted-1.png` (and two tabs in the same session can likewise both paste `pasted-1.png`).
When leaving the session, `post_departed_uploads` (`attachments.rs:429-441`) constructs
`Notice::InterruptedUpload(Rc::new(InterruptedUploads::new(key, title, files, sent)))` with no unique occurrence
identifier and calls `notices::post`. `notices::post` (`crates/gui/farhelm-gui/src/notices.rs:177-186`) ignores any
notice already present in `notices.posted` by `PartialEq` (`InterruptedUploads` derives `PartialEq` over
`(session, title, files, sent)` at `notices.rs:93-99`). As a result:

1. Returning to the session while the first notice is undismissed, pasting a brand-new screenshot (`pasted-1.png`) or
   retrying the same file, and leaving before completion silently drops the new interruption (and `MainView::render` at
   `main_view.rs:188-190` sees `self.drawn_banners.contains(last)` so it does not scroll the banner stack either).
2. If two terminals of the same session both have `pasted-1.png` in flight on departure and `post_departed_uploads` runs
   between their two `hand_over` calls, the second is silently deduplicated. Also note that `InterruptedUploads::key()`
   (`notices.rs:118-122`) hashes `self` via `DefaultHasher` (64-bit) to form element IDs (`notice-interrupted-upload`,
   `notice-dismiss-interrupted-upload-<key>`); adding a monotonic `u64` occurrence ID to `InterruptedUploads` (used in
   `PartialEq`, `Hash`, and `key()`) fixes both silent deduplication and any hash-collision ID reuse. Fix direction:
   Give each posted `InterruptedUploads` a monotonic per-app sequence id (or merge/replace and reveal in
   `notices::post`) so distinct interrupted uploads are never silently dropped.

Reviewer metadata (g19-uploads-terminal-attachments/f2): suggested bucket: other (not highest: local files are untouched
and no user-owned data, credentials, or processes are lost; however, a second interrupted upload—especially a different
pasted screenshot that reuses the generated name `pasted-1.png`—is silently dropped when the prior notice is still
open); confidence: confirmed; possible cover: none (cross-group interface with g20-uploads-notices-main-view:
`notices.rs` defines `InterruptedUploads` equality and `notices::post` deduplication, while `attachments.rs` resets the
`pasted-1.<ext>` counter per terminal and builds `InterruptedUploads` without a unique occurrence id).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g20-uploads-notices-main-view/f1`,
`g19-uploads-terminal-attachments/f2`.
