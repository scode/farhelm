# The full Model picker selects Antigravity without offering hook setup

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Choosing an Antigravity model in the full Model picker skips the hook setup offer shown for the equivalent command-field
choice.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/model.rs:81-118;
launcher/view/model_field.rs:86-99,166-238,255-315; launcher/draft.rs:693-711; launcher/view/search.rs:440-461;
crates/farhelm-helm/src/launches.rs:108; SPEC.md:1085-1101,3092-3096

Explicitly choosing an Antigravity model from the launcher's dedicated full Model list changes the agent to Antigravity
but skips the first-pick hook question. Model options include all catalog agents when Show all is selected. A picked
model reaches pick_model by button or Enter and choose_catalog_model switches to the model's owner. That handler closes
the list and calls chose only. Selecting the same foreign model through the launcher command field performs the
equivalent edit and then explicitly calls check_hook_question on agent change. The source catalog includes Antigravity
models, and the specification calls switching via the full list deliberate. No setup or dismissal is written from this
path and later session hints remain available. Suggested other for missing the first-pick offer. Confirmed from source,
applies to the Antigravity model route rather than Grok, which has no catalog models. ; distinct from tab retention and
catalog highlight issues.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-launcher/HL-14`.
