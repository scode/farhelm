# check_template_shape skips validate_model_or_effort on template.fields.model when agent is Custom

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

Saving a custom-agent launch template from the CLI or API accepts invalid model values (such as uppercase letters,
spaces, control characters, or empty strings) even though Farhelm refuses those same values in the desktop Templates
editor and whenever the template is applied.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/launcher.rs:560-581`.

Per SPEC.md (lines 493-494) and website/src/content/docs/docs/agents/custom-agent-contract.md (lines 51-56), custom
agent model and effort values must be 1 to 256 bytes from [a-z0-9_-] (validated by
crate::custom_agents::validate_model_or_effort). Both apply_template
(crates/farhelm-proto/src/launcher.rs:973-975,1016-1018) and GUI Field::refusal
(crates/gui/farhelm-gui/src/templates/fields.rs:243-259) enforce validate_model_or_effort on both model and effort when
agent.uses_custom_catalog(). However, check_template_shape (crates/farhelm-proto/src/launcher.rs:560-581) only calls
validate_model_or_effort on template.fields.effort (line 580) and never validates template.fields.model when
template.fields.agent is Some(LaunchHarness::Custom). As a result,
`farhelm agent template create foo --agent acme-code --model 'BAD MODEL'` (crates/farhelm/src/main.rs:1117-1120 passes
raw model text without a parser; crates/farhelm-helm/src/agent_requests.rs:3351 relies on check_template_shape) and PUT
/api/templates/{name} succeed and persist an unapplicable template.

How to verify: Call check_template_shape on LaunchTemplate { name: "bad-model".into(), fields: TemplateFields { kind:
Some(LauncherKind::Agent), agent: Some(LaunchHarness::Custom), custom_agent: Some(Some("acme-code".into())), model:
Some(Some("BAD MODEL\n".into())), ..Default::default() } }; it returns Ok(()), whereas apply_template refuses it.

Fix: In check_template_shape (crates/farhelm-proto/src/launcher.rs:560-581), when
template.fields.agent.is_some_and(LaunchHarness::uses_custom_catalog) and template.fields.model is Some(Some(model)),
validate model with crate::custom_agents::validate_model_or_effort(model).map_err(str::to_owned)?.

Reviewer metadata (g41b_custom_agents_proto_launch/f6): suggested bucket: other (validation-boundary gap at template
save time; apply_template and launches::compile_custom still refuse the invalid model before launch, so no security or
data-loss impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41b_custom_agents_proto_launch/f6`.
