# The final-frame boundary test leaves OSC 8 and file paths unasserted

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

Existing exit tests would miss a regression that joins a file path or combines OSC 8 labels across the
retained-history/final-frame boundary.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-grid-model/tests/model.rs:880`.

the new model test at model.rs 880–925 checks a printed URL via link_at, logical-line ranges and clipboard text. The new
GPUI assertion at attached/tests.rs 3003–3073 checks navigation and Select All, and the mouse test at 3874–3905 checks a
plain drag and bare motion. They do not exercise path_links or link_disclosure with an exited alternate screen, so
removing the separate break guard in paths.rs 429 or either neighbor check in link_disclosure.rs 224–251 could leave
those exit tests passing. The ordinary path test at tests/paths.rs 309 starts with a same-buffer path. The contract at
SPEC.md tip 3301–3304, 3505–3514 and 3549–3565 depends on the separate buffers keeping their own labels and targets.
Assert that the guards stop both directions at the boundary while retaining joins within a buffer. Bucket other
(regression coverage). Confidence confirmed by inspecting existing assertions, with no runtime result or current
mis-link claimed. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/f1`.
