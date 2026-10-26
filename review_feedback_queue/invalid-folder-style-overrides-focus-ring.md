# Invalid folder styling overrides its keyboard focus ring

Reviewed commit: 1de332971ec47040f095b0fbc32903fbcb88ae73

## TLDR

After a folder refusal, the focused editor uses the same outline as the blurred invalid field. CSS specificity
establishes the missing distinct ring; practical accessibility impact was not measured.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/assets/app.css:6930`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Reviewed commit: 1de332971ec47040f095b0fbc32903fbcb88ae73. TLDR: after a folder refusal, the folder editor no longer
shows the application's distinct keyboard-focus ring, making focus harder to locate while correcting the failed launch.
Exact original references: app.css:2006-2023 establishes the shared keyboard ring as outline:2px solid var(--accent)
with offset:1px;6930-6933 introduces .launch-composer-folder-picker input[aria-invalid=true] with outline:1px solid
var(--danger) and offset:-1px. The new selector is more specific and applies both focused and blurred, so it replaces
the shared ring throughout the invalid state. create_form.rs:6435-6455 connects that selector and aria-invalid to the
real folder input. app.css:6021-6038 supplies its ordinary border/background but no replacement focus treatment; the
bounded focus-selector search found no input-specific rule restoring the ring. Use a border/inset shadow for invalid
state, or preserve an explicit :focus-visible rule alongside it. Suggested bucket: other. Confidence: confirmed CSS
cascade and shipped trigger. Caveats: source-only, no rendering/accessibility measurement; caret and folder suggestions
can remain visible, so this is loss of the distinct focus ring, not proof of total inability to locate focus or operate
the field. No security or work-loss consequence. possible_cover: none matching found in pinned SPEC.md, SPEC_impl.md,
TODO.md Planned, BUGS.md, TRIAGE_OUTCOMES.md, queue/index or plans; SPEC_impl.md:817-822 says modal controls retain
keyboard rings. proposed_drop: none. The trigger is an ordinary failed folder launch plus keyboard focus, not a
demonstrated rare timing/interruption condition for FILTER.

The pending Steel foundation/dialog plans restyle shared controls and retain keyboard behavior. They do not explicitly
remove or override this more-specific invalid-input outline selector; generic visual-review acceptance is not treated as
exact cause-removal coverage. Triage can decide whether the planned CSS work covers the remainder.

Provenance: `g243/f4`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
