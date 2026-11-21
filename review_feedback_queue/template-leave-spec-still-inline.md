# Templates implementation spec still describes the old inline leave prompt

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The implementation spec still describes the Templates leave question as inline, although the app centers it.

## Details

f758e3ace8bf2161954f2a54392353448ab41171 changes the question from prompt::ask_inline to prompt::ask, removes the
footer's inline question, and updates the general prompt paragraph that now appears at SPEC_impl.md:1018–1025.
7223ac49de6d3d5a67b6aceb1c15b4f75d6646a8 also requires the draft to be dirty, so switching an inferred legacy draft back
to its raw stored value leaves without asking. The endpoint product contract is SPEC.md:1272–1277. It matches
crates/gui/farhelm-gui/src/templates/editor.rs:406–417 and :915–920, and templates/dialog.rs:417–440 and :529–603. The
scope-specific implementation paragraph at SPEC_impl.md:8929–8931 still calls the prompt inline and omits the
two-baseline predicate. No later in-range commit changed that paragraph to match. Update it to describe the centered
question, the narrower leave rule, and the distinction between ordinary Save and Save followed by a successful departure
(templates/dialog.rs:500–516).

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `templates/f1`.
