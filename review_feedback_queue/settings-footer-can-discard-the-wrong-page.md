# A shown footer Discard can discard a different Settings page

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A save refusal can switch pages before a stale Discard press runs, making it discard a different draft.

## Details

A footer Discard callback is keyed to the shared dialog and rereads the current section
(`crates/gui/farhelm-gui/src/settings/editing.rs:283–321`). A pending save failure can change that section before the
next frame is shown (`settings/editing.rs:438–450,526–543`). This creates a source-supported distinct route to
discarding the refused draft from the wrong visible footer. The actual effect depends on previous-frame delivery before
redraw. Bind the action to the shown section. The existing single-page test presents a fresh frame before invoking
Discard (`settings/tests.rs:3235–3255`, `crates/gui/farhelm-gui/src/test_support.rs:264–270`) and cannot detect a page
transition that changes the target.

Confidence: likely, with the named premise that the refusal changes the page after the previous footer was drawn and
before that frame's click is delivered.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l9`, `footerTests/l8`, `footerTests/t3`.
