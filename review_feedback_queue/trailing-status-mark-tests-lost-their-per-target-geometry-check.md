# Status-mark tests lost their per-target geometry check

Reviewed commit: 2dc0555c3db4a6d776b6ca5a9cc20ca6dc068f90

## TLDR

The row status tests verify that each mark's help appears with a notch and no ring. They can pass if placement uses the
bounds of a different element in the same sidebar.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/summary_tests.rs:480`.

SPEC.md:2771–2774 requires a notch pointing to the target's center, clamped to the card. In the parent,
summary_tests.rs:405–437 and :488–530 used the ring rectangle as a per-mark check of the tracked bounds. Today's status
helper at :480–485 checks help text, beside placement, notch presence and no ring. host_rail/tests.rs:526–539 also
checks horizontal clearance, without checking notch height. This does not remove all geometry coverage.
layers/beside_tests.rs:337–361 tests clamping, and :647–691 asserts the real notch is level with a button.
goldens/session_list.rs:330–351 and tests/goldens.txt:217–218 cover one age-mark scene by whole rendered output. The
exact gap is binding each affected row mark and the rail status to its own target geometry. beside.rs:157–183 and
:274–301 reuse the common placement logic, and :662–688 draws the notch independently of ring painting. Keep the
existing assurances and assert placement against each status target's bounds, accounting for the documented clamp.
Bucket: other, maintainer test coverage. No shipped placement error established. Confidence: confirmed by pinned source
and assertions. Possible coverage: session-row-hover-beside Linux goldens cover one age-mark presentation. Generic
geometry tests cover a button and clamping, rather than each status mark's binding.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `status/f5`.
