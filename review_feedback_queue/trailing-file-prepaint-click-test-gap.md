# The pre-paint file click test misses the placed-but-unpainted window

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The test meant to pin that a click on a terminal file before its details card is drawn does nothing does not exercise
the moment the card has been placed but not yet drawn. If the code stopped requiring a drawn card at the press, the test
suite would not notice. The visible effect of such a regression would be a spurious "move the pointer and hover again"
refusal; no file would be fetched, because a separate visibility check at the press still fails.

## Details

Concerns 3ed37d4c20 (test rewritten) and 5fd36869f5 (kept the SPEC.md sentence "A click before the disclosure paints
does nothing" and added a new sentence about the same painted disclosure). Test:
`crates/gui/farhelm-gui/src/terminal/files/tests.rs:719-765` (`a_click_before_the_answer_has_painted_stays_silent`).
Guard under test: `crates/gui/farhelm-gui/src/terminal/attached.rs:4092-4101` (the `LinkPress.file_disclosure` filter
`shown.drawn.get().is_some()`).

The test answers the check and dispatches press and release inside one window update, and asserts
`owner.read(cx).disclosure_drawn().is_none()` as its premise. In that interval `AttachedTerminal.disclosure` is None,
because the disclosure record is only created by `place_disclosure` during render (`attached.rs:3166-3178`). The
press-time filter therefore rejects on `self.disclosure.as_ref()` being None and never reaches the
`shown.drawn.get().is_some()` clause. The real "before paint" window that clause exists for comes after
`place_disclosure` created the record with an empty Drawn, and after the layer is opened through `window.defer` once
that frame is done (`attached.rs:3182-3195`), but before the popover paints and fills `drawn`. Removing the
`drawn.get().is_some()` clause would leave this test green.

In that regression a press in the placed-but-unpainted window would get `file_disclosure = Some(generation)`.
`press.visibility` from `click_visibility` (`attached.rs:3319-3342`) is `Err(NotVisible)` because
`disclosure::visibility` (`disclosure.rs:115-116`) fails on an empty Drawn. At release `FileLinks::clicked`
(`files.rs:212-243`) maps `(Err(NotVisible), Err(_))` to a refusal message and `(Err(NotVisible), Ok)` to silence: no
transfer, but a spurious refusal where SPEC.md says the click "does nothing". Before this commit the old test asserted
the answer reset `drawn` of a checking display that really had painted; with the checking display gone, the rewrite fell
back to the trivial interval.

No other GUI test covers the placed-but-unpainted interval: `disclosure_drawn` is used only by this test (as a None
premise), by `stage_off_window` (`files/tests.rs:1185-1196`, which requires an already-drawn panel) and by
`attached/tests.rs:5505` (a web link, after paint). SPEC_impl.md:3485 states the guarded rule ("A click requires a
painted disclosure at press"), and this test is the only one positioned to pin it.

Fix: stage the interval explicitly. Answer the check, run one render so the Disclosure record exists with an empty Drawn
(assert `disclosure_drawn()` is Some and its `get()` is None), then press and release inside the same update before the
popover paints, and assert no admission and no refusal.

Bucket: other (the test fails to pin a guard whose regression yields only a wrong message, not a transfer). Not highest:
the press-time visibility check independently blocks the transfer.

Confidence: likely. The unverified premise is that GPUI dispatches a mouse event between the deferred layer open and the
popover's first paint. The deferral structure implies it (`place_disclosure` defers `open_disclosure` to after the
frame, so the layer paints at the earliest in the next frame), but no source line states it outright.

Possible cover: none known.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g10 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
