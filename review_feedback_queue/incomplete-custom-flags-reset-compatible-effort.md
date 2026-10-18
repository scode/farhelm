# Incomplete permission flags reset a compatible reasoning effort

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Accepting a custom model while permission flags contain an unfinished quote can reset a compatible reasoning effort to
default, even when the model has not changed. The explanation incorrectly attributes the reset to model compatibility.

## Details

The custom-model Enter branch checks validity of the entire selection to decide whether to clear effort
(`crates/farhelm-ui/src/list/create_form.rs:6682–6693`). `launch_composer.rs:1465–1474` rejects malformed Custom flags
before testing effort. An incomplete field unrelated to effort therefore resets it. Use an effort-specific compatibility
check instead. This affects New, Clone and Replace with; Restart with already uses an effort-only check
(`restart_with.rs:433–439`).

Suggested bucket: **other**. Confidence: **confirmed static control flow**; no runtime reproduction. The effect is a
changed unsaved launch choice, not demonstrated loss of work. The finding was discovered in directly relevant caller
context outside that reviewer's mandatory source ranges.

Possible cover: `custom-permission-model-choice-erases-draft.md` in TRIAGE_OUTCOMES.md concerns lost flags through
search, not lost effort through custom-model Enter. SPEC.md's launch-composer rules and SPEC_impl.md:4739 require
dropping incompatible effort, not compatible effort because a different field is incomplete. No matching Planned, BUGS,
queue or filter coverage was found.

Proposed drop: none. Provenance: persistent Astra/high review, `ui-launch-workflow-1/f3`.
