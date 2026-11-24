# Guarded plain Replace reads live store state at click time instead of binding to the drawn frame, and silently drops answers when the row is absent

Reviewed commit: 6d32efc909d5bf48a7bef0a1dc574d4ada0b2e5a

## TLDR

Answering "Replace as guarded instead" on a plain Replace YOLO confirmation reads the session's live store entry at
click time rather than what was drawn on screen, so an undrawn update can change the replacement's title or directory,
and if the row left the filtered session list right before the press the question closes and does nothing with no
notice.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/replace_yolo.rs:215-244`.

In crates/gui/farhelm-gui/src/session_list/replace_yolo.rs:204-255 (at 6d32efc909d5bf48a7bef0a1dc574d4ada0b2e5a and
unchanged at tip 1b3fe49af86979aebc2719965682a17f8c6366ac), replace_yolo::ask freezes host, asked_launch,
expected_incarnation, and DeleteGuard when the YOLO question opens, but its Guarded answer callback (lines 215-244)
calls `store.read(cx).collections().get::<SessionRow>(&key)` directly when the user clicks or presses Enter and passes
that live row to `guarded_create(row, expected_incarnation)` (lines 283-304).

Three concrete defects follow from this design:

1. Drawn-frame binding violation (SPEC_impl.md, "Questions: the prompt", lines 982-997): Every confirmation answer must
   act on the value presented on screen when answered (either frozen when the question opens or updated through
   `Inline::refresh_value` on draw, which bumps `prompt.wording` so answers arriving before a frame shows the new state
   are gated by `wording != drawn_wording` in crates/gui/farhelm-gui/src/prompt.rs:831, 844, 1387-1397). Because
   `replace_yolo.rs:219` reads live store state in the callback without going through `YoloAsk` / `refresh_value`, a
   pushed `SessionRow` update that has arrived in the store but has not yet been drawn supplies its undrawn
   `row.info.title` and `row.info.cwd` under GPUI's queued-input delivery. In addition, line 224 checks
   `row.host != host || row.info.launch != asked_launch` but does not check `row.info.cwd`.
2. Silent drop when the row is absent (lines 219-221): `prompt.rs:858-859` sets `self.answered = true` and closes the
   inline question before running `send_again`. If `collections.get::<SessionRow>(&key)` returns `None` (for example, a
   non-selected row transitioning out of a status/search filter right before a queued click or Enter, or a concurrent
   remote deletion), `send_again` executes `return;` with no HTTP request and no `operations.note(...)`. By contrast,
   `ReplaceAnswer::Yolo` (lines 214, 246) still dispatches `POST /api/sessions/{key}/replace`, and a host/launch change
   at line 224 posts `operations.note("This session changed while the question was open. Choose Replace again.")`.
   Neither `SPEC.md:1578-1582` nor `SPEC_impl.md:5223-5227` documents a silent drop.
3. Spec contradiction in `SPEC_impl.md`: lines 987-989 ("Questions: the prompt") assert "One confirmation does not
   freeze: the launcher's YOLO question keeps its warning about the source session live while it is open, and its answer
   acts on the warning it displayed when answered", which contradicts `SPEC_impl.md:5223-5227` (added in commit
   42996eb8), which states that plain Replace's guarded answer sends "the visible source title and directory at answer
   time".

How to verify: Inspect `replace_yolo.rs:204-255, 283-304` alongside `prompt.rs:831-868, 1387-1397` and
`SPEC_impl.md:982-999, 5223-5227`. In a GPUI test, open plain Replace's YOLO question on row `a`, push a `SessionRow`
removal (e.g., filtered out) or a rename without redrawing, click `prompt-alternative-0`, and observe either silent
no-op (zero requests, zero refusal note, closed prompt) or submission of an undrawn title/cwd.

Fix: Freeze the drawn `(cwd, title, host, launch, expected_incarnation)` snapshot in the question value (`YoloAsk` or a
Replace-specific frozen payload) when `replace_yolo::ask` opens; if concurrent renames should still be carried over when
visibly drawn, propagate the drawn `SessionRow` fields into the prompt on draw via `Inline::refresh_value` (or freeze
`cwd`/`title` at open as plain Replace's confirmation already does for its title) so the answer always acts on the drawn
snapshot and never silently drops when a non-selected row leaves a filtered listing. Reconcile `SPEC_impl.md:987-989`
and `5223-5227` to match.

Reviewer metadata (g21_yolo_guarded_replace_gui/l1): suggested bucket: other (consequence rationale: `host`,
`expected_incarnation`, `asked_launch`, and `DeleteGuard` remain frozen, no shipped path mutates a session's `cwd` after
creation, and the missing-row branch fails closed as a no-op rather than deleting or launching YOLO; this is a
correctness / drawn-frame-binding and UX-feedback defect rather than a security or data-loss bug. It would move to
`highest` only if a shipped path could mutate `row.info.cwd` or reuse a session key for another directory/target before
redraw, which is not established in shipped code).; confidence: confirmed (established by source and spec analysis; not
executed).; possible cover: none.

Additional source record at `42996eb829d605bae99e03209a6f7ccc2e3d4478`, `SPEC_impl.md:5227-5231`
(g21b_yolo_guarded_replace_spec/f1):

- In SPEC_impl.md:5227-5231 (at review tip 1b3fe49af86979aebc2719965682a17f8c6366ac; added by
  42996eb829d605bae99e03209a6f7ccc2e3d4478), the new paragraph states: "Plain Replace uses the shared YOLO question. The
  guarded answer sends the existing Replace-with body with the visible source title and directory at answer time, the
  original host incarnation and Delete guard, and a Guarded selection. ... If the visible source moves hosts or its
  saved launch changes before the answer, the desktop asks the user to choose Replace again." SPEC.md:1588 likewise
  states: "It keeps the source host, displayed directory and title, and the original Delete precondition."
- Reconciled with `g21_yolo_guarded_replace_gui`'s citations
  (`crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs:1150-1174` and `SPEC_impl.md:998`): carrying a concurrent
  rename that has been drawn on screen is intentional (`lifecycle_tests.rs:1150-1174`), and `SPEC_impl.md:998` permits
  live state to _refuse_ a missing or unavailable target ("Live state may also refuse a missing or unavailable target").
  However, three real spec/implementation contradictions remain:
  1. Direct internal contradiction in SPEC_impl.md: "Questions: the prompt" (SPEC_impl.md:984-991) states that "When a
     confirmation opens, it freezes what it shows: targets, consequences and preconditions. Its answer buttons come
     pre-wired to that frozen state, so feature code never wires an answer by hand" and explicitly states "One
     confirmation does not freeze: the launcher's YOLO question keeps its warning about the source session live while it
     is open, and its answer acts on the warning it displayed when answered." SPEC_impl.md:5227-5231 introduces a second
     un-frozen confirmation answer without updating "Questions: the prompt".
  2. Live store payload read vs drawn-frame binding: Immediately after allowing live state to refuse a
     missing/unavailable target, `SPEC_impl.md:999` mandates "The question and request use the snapshot that admitted
     the press." Unlike the launcher's YOLO question (which updates its prompt value through `inline.refresh_value` on
     draw in `crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:95-101`),
     `crates/gui/farhelm-gui/src/session_list/replace_yolo.rs:215-244` reads live
     `store.read(cx).collections().get::<SessionRow>(&key)` inside the `ReplaceAnswer::Guarded` callback and constructs
     `CreateSessionBody` (`title`, `cwd`) from that live store entry rather than from a drawn snapshot. Under GPUI's
     dirty-frame/queued-input model (`SPEC_impl.md:997-1002, 1045-1046`), a store update not yet rendered is neither
     "visible" (`SPEC_impl.md:5227`) nor "displayed" (`SPEC.md:1588`), and `replace_yolo.rs:224` checks only
     `row.host != host || row.info.launch != asked_launch`, not `row.info.cwd`.
  3. Missing-row behavior undocumented in SPEC_impl.md:5230-5231: while `SPEC_impl.md:998` allows refusing a missing
     target from live state, `SPEC_impl.md:5230-5231` names only moving hosts (impossible under an entry-UUID-prefixed
     session key on an honest helm) or a changed saved launch (which posts "This session changed while the question was
     open. Choose Replace again." at `replace_yolo.rs:224-239`), omitting that a row absent from `collections` at answer
     time (`replace_yolo.rs:219-221`) silently closes the question without dispatching or posting a refusal.
- How to verify: Compare `SPEC_impl.md:984-1005` against `SPEC_impl.md:5227-5231`, `SPEC.md:1585-1589`,
  `crates/gui/farhelm-gui/src/session_list/replace_yolo.rs:189-304`, and
  `crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs:1118-1183` at `1b3fe49af86979aebc2719965682a17f8c6366ac`.
- Fix: Either bind guarded plain Replace to drawn-frame state (refreshing the prompt's frozen `title` on draw as
  `YoloState::show_source` does while freezing `cwd` at question open) and update `SPEC_impl.md` "Questions: the prompt"
  to cover both live-refreshed YOLO confirmations, or reconcile `SPEC_impl.md:984-999`, `SPEC_impl.md:5227-5231`, and
  `SPEC.md:1588` to accurately describe the live store read and missing-row handling.

Reviewer metadata (g21b_yolo_guarded_replace_spec/f1): suggested bucket: other (spec contradiction and
drawn-frame-binding mismatch in a confirmation handler; on an honest supervisor `sessions.cwd` is immutable after launch
(`crates/farhelm-supervisor/src/store.rs:4213` only updates `cwd` while `outcome_state = 'launching'`) so only `title`
changes via concurrent rename. The separate untrusted-remote-supervisor `cwd` trust-boundary question is elevated to
`c4` with `proposed_drop` for the independent drop check).; confidence: confirmed (verified from source and spec at
1b3fe49af86979aebc2719965682a17f8c6366ac; no tests executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g21_yolo_guarded_replace_gui/l1`,
`g21b_yolo_guarded_replace_spec/f1`.
