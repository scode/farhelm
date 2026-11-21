# A model catalog reply can change the meaning of the highlighted Show all option

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

An arriving model catalog can turn Enter on the highlighted Show all option into a model selection.

## Details

With a harness selected in New, model_options puts HarnessDefault first, catalog models next and ShowAll last
(crates/gui/farhelm-gui/src/launcher/model.rs:84–117). The model field shows ShowAll before the catalog arrives and
Arrow can select its index (crates/gui/farhelm-gui/src/launcher/view/model_field.rs:85–98, :149–162). A successful
catalog reply replaces the rows but resets only the composer’s highlight
(crates/gui/farhelm-gui/src/launcher/view.rs:480–495). model_enter passes the old explicit index and newly computed rows
at model_field.rs:178–184, and model.rs:167–172 gives that index priority over typed text. A model inserted ahead of
ShowAll takes its position and can be accepted before the new frame is shown (SPEC.md:5500–5505 and
SPEC_impl.md:984–988). Keep the actively selected option by identity when the catalog changes, and accept only an option
in the presented frame.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `templates/l6`.
