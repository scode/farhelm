# Empty session titles emit malformed upload notices and batching can drop a later upload's title

Reviewed commit: e3d03d5651e9f0b417514e145ce847a1e45fffe8

## TLDR

When an upload is interrupted in a session with an empty title, the window notice reads 'Attaching <file> to was
interrupted…' with a dangling 'to' and double space. Separately, if a first upload started before the session row loaded
and a later upload in the same batch captured the session title, the notice drops that title.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal/attachments.rs:585-591`.

Two title-handling issues in `crates/gui/farhelm-gui/src/terminal/attachments.rs`:

1. Empty titles: The supervisor and helm explicitly allow an empty string as a valid `SessionInfo.title` on create and
   rename (`crates/farhelm-supervisor/src/service/core.rs:3964-3976`,
   `crates/farhelm-helm/src/sessions_tests.rs:7029-7030`). `Attachments::attach` (`attachments.rs:585-591`) maps
   `row.info.title.as_str()` unconditionally into `Some(HostText::from_host(...))` without filtering
   `.filter(|title| !title.is_empty())` (compare `crates/gui/farhelm-gui/src/launcher/pill.rs:92-94`).
   `InterruptedUploads::sentence` (`crates/gui/farhelm-gui/src/notices.rs:141-143`) emits `.text(" to ").host(title)`
   whenever `self.title` is `Some`, yielding `"Attaching <file> to  was interrupted..."` instead of omitting the title
   clause as SPEC.md, "Attachments" (line 3642: 'when its title is available') calls for.
2. Dropped title across batched uploads: `post_departed_uploads` (`attachments.rs:426-428`) takes
   `let title = first.title.clone();` from only the first upload in the `sent` or `unsent` partition. If `first.title`
   is `None` (e.g., attached before the session's row arrived in the store) while a subsequent upload in the same
   partition captured `Some(title)`, or the row is now present in `session::current_store(cx)`, the available title is
   discarded. Fix direction: Filter out empty titles in `Attachments::attach` (`.filter(|title| !title.is_empty())`) and
   in `post_departed_uploads` pick the first available non-empty title from `uploads` (falling back to
   `session::current_store(cx)` if still present).

Reviewer metadata (g19-uploads-terminal-attachments/l3): suggested bucket: other (not highest: wording/formatting defect
with no security or data-loss impact); confidence: confirmed; possible cover: none.

Additional source record at `e3d03d5651e9f0b417514e145ce847a1e45fffe8`, `crates/gui/farhelm-gui/src/notices.rs:141-143`
(g20-uploads-notices-main-view/f3):

`InterruptedUploads::new` (`crates/gui/farhelm-gui/src/notices.rs:103-115`) stores `title: Option<HostText>` verbatim,
and `InterruptedUploads::sentence` (`notices.rs:141-143`) appends `.text(" to ").host(title)` whenever `self.title` is
`Some(_)`, without checking whether the title is empty. The sole production caller
(`crates/gui/farhelm-gui/src/terminal/attachments.rs:585-591`, cross-group `g19` `l3`) maps `row.info.title.as_str()`
directly into `Some(HostText::from_host(...))` without filtering empty strings, and `SessionInfo.title`
(`crates/farhelm-proto/src/lib.rs:1045`) is an unvalidated `String` that can be empty (e.g.,
`crates/farhelm-helm/src/sessions_tests.rs:7168` notes empty titles are accepted on rename and attached hosts supply
`SessionInfo.title`). Contrast `crates/gui/farhelm-gui/src/launcher/pill.rs:92-94`, which filters
`.filter(|title| !title.is_empty())` before wrapping in `HostText::from_host`. Under SPEC.md ("Attachments", lines
3641-3642), the notice names the session "when its title is available". Fix direction: normalize
`title: title.filter(|t| !t.raw().is_empty())` in `InterruptedUploads::new` (and/or guard `!title.raw().is_empty()` in
`sentence()` and filter in `attachments.rs:590`). Use `!t.raw().is_empty()` rather than `!t.raw().trim().is_empty()` so
titles containing escaped control characters like `\t` (`<U+0009>`) remain visible instead of being silently dropped.

Reviewer metadata (g20-uploads-notices-main-view/f3): suggested bucket: other (could be in `highest`: no — malformed
notice prose only, no security or data-loss impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g19-uploads-terminal-attachments/l3`,
`g20-uploads-notices-main-view/f3`.
