# The template editor conceals host-supplied characters

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

Saving a cloned session as a template exposes editable model and folder values whose visible spelling can differ from
the bytes Farhelm retains and later launches.

## Details

Suggested bucket: `highest`, conservatively for misleading presentation across the host trust boundary. Confidence:
confirmed by code inspection; no browser reproduction or arbitrary-execution consequence established. Source:
adversarial launcher reviewer. Proposed drop: none.

`crates/farhelm-ui/src/list/templates.rs:601` seeds the model input with raw stored model text; `706` does the same for
the destination. Model suggestions at `611` also interpolate raw IDs. These controls do not use escaped display with an
untouched raw seed. The new save flow copies the launcher's model and destination (`list/save_template.rs:43–55`), then
immediately opens that saved template (`list/view.rs:3604–3611`). The remote-clone → save → editor route was added by
`69ebbac6c9e158fa9e94be6f7a193e1d37d3b64c`.

Validation does not remove the trigger. `crates/farhelm-proto/src/launcher.rs:347–363` excludes model and destination
from presentation-character refusal. `crates/farhelm-helm/src/launches.rs:605–620` refuses model controls and whitespace
but allows formatting characters such as U+200B and U+202E. A remote-derived model ending in U+200B can therefore look
like the model without that character; folder values can conceal characters too. A single-line input cannot faithfully
display embedded line breaks either.

Clone a remote session with such a custom model or folder, save the relevant choices, and inspect the automatically
opened editor. Verify visible escapes, unchanged raw-byte preservation when saving another field, and edited-byte
semantics after actual input. Apply the existing escaped-display/raw-seed/edited-flag treatment; do not persist escaped
display text as though it were the original choice.

Possible cover: TRIAGE_OUTCOMES.md's `template-summaries-interpolate-model-text-into-approval-*` findings concern
summary rendering, while `rename-editor-conceals-title-characters-editable-title-editable-title-seed.md` concerns
Rename. Neither covers these editor controls. SPEC.md:2333–2337 explicitly requires visible escaping outside terminals
and names a model saved into a template from a cloned session; SPEC_impl.md:633–639 explains the editable-control
pattern.
