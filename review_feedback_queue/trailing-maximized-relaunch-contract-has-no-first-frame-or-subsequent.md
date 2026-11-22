# The maximized relaunch contract has no first-frame or subsequent native Zoom assertion in the checked fixtures

Reviewed commit: 93b0cd7386d134c3b480bb96f0d7efb9aec42820

## TLDR

The checked cases could pass if a maximized Mac relaunch still flashed the ordinary frame or if the next Zoom returned
to a different rectangle.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/bootstrap.rs:443`.

SPEC.md:3884–3890 promises both outcomes. crates/gui/farhelm-gui/src/bootstrap.rs:430–455 passes a synthetic ordinary
rectangle with WindowBounds::Maximized and asserts WindowOptions.window_bounds before native opening; :462–474 asserts
that test-agent sizing instead intentionally selects Windowed. crates/gui/farhelm-gui/src/desktop/tests.rs:412–436
asserts host-answer translation and :557–565 asserts the two-point threshold against literal sizes, including the
earlier one-point-short case. No one of those tests samples the native window. The FrameKeeper tests at
crates/farhelm-desktop/src/window_state.rs:905–935,943–962,970–985 exercise real storage and transition rules with
synthetic Observed values. They supply both the maximized confirmation and, where applicable, the post-unzoom rectangle
as inputs. They do not assert AppKit generated either. The title-band gesture test asserts PressAction::Zoom
(desktop/tests.rs:925–941). The native window-drag contract does measure zoom and restore geometry for an already open
ordinary window (releasing/mac-vm-test/checkpoint_contract.py:311–341), including comparison to the immediately
pre-gesture frame. It has no assertion of first visible geometry or comparison to a frame remembered across relaunch.
Consequently these existing fixtures cover distinct pieces of the path and omit the new macOS startup boundary at
vendor/gpui-pre-macos/src/window.rs:1247–1254,1336–1340. The assigned commits change no fixture or assertion. The
finding is a source-supported regression-coverage omission, not evidence that a current build violates either behavior.
An assertion covering the contract would bind the supplied ordinary rectangle and maximized request to the first visible
native geometry and the next native Zoom's restored rectangle. A settled-window assertion alone would miss the opening
flash. Bucket: other (test coverage). Confidence: confirmed by source for the checked assertions and acceptance
contract. No runtime observation. Possible coverage: window-drag covers only the in-window gesture and matching ordinary
frame within one window lifetime. The pinned executable oracle also confirms the boundary:
releasing/mac-vm-test/window_geometry.py:89–116 requires one process/window identity across all ten sampled image
states, greater area after the title-band Zoom, and equality of x, y, width and height with the corresponding
pre-gesture state after the repeat. Those comparisons cannot span a relaunch, and the enlarge comparison does not
independently require the display’s usable area. The cited fixture and assertion files are unchanged from
c83d5c61409756a0f8af77c247025c331425655d through 75edc318c2415167d65a8d472c1127fcc58c1cbf.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `zoom/z3`.
