# TemplateFields::implied_kind ignores custom_agent, causing a kindless custom_agent-only template to open under Don't switch with an incompatible Agent row

Reviewed commit: 0ae1f6de79be7b712ec33dfd71b3a38edf3b1399

## TLDR

A launch template saved through the API with only a custom agent ID (without also setting launch kind or agent type)
does not infer a launch kind when opened in the desktop Templates editor, causing it to display under "Don't switch"
with a bogus "registration unavailable" agent row that refuses saving.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/launcher.rs:464-472`.

check_template_shape (crates/farhelm-proto/src/launcher.rs:560-569) permits saving a template with custom_agent:
Some(Some("acme-code")) and agent: None, kind: None (reachable via PUT /api/templates/{name} or a raw agent wire
request; shipped CLI and GUI controls always set agent alongside custom_agent). However, TemplateFields::implied_kind
(crates/farhelm-proto/src/launcher.rs:464-472) checks self.agent.is_some() || self.sets_agent_only_fields() and omits
self.custom_agent.is_some(), returning None. In the desktop Templates editor, Draft::from_template
(crates/gui/farhelm-gui/src/templates/fields.rs:430) leaves fields.kind = None ("Don't switch"), while
Field::Agent::is_set (fields.rs:120) checks fields.agent.is_some() || fields.custom_agent.is_some() -> true. Because
Field::Agent::offered (fields.rs:203) is false when kind is None, the editor shows an incompatible Agent type row whose
fallback in templates/editor/rows.rs:312 pairs LaunchHarness::BUILT_INS[0] (Cursor) with Some("acme-code") and renders
"acme-code (registration unavailable)".

How to verify: Call TemplateFields { custom_agent: Some(Some("acme-code".into())), ..Default::default()
}.implied_kind(); it returns None even though check_template_shape accepts it and Field::Agent::is_set returns true.

Fix: Either include self.custom_agent.is_some() in TemplateFields::implied_kind
(crates/farhelm-proto/src/launcher.rs:467) to match Field::Agent::is_set, or reject custom_agent without agent ==
Some(LaunchHarness::Custom) in check_template_shape if custom_agent-only modifier templates are not intended.

Reviewer metadata (g41b_custom_agents_proto_launch/f5): suggested bucket: other (narrow API-only edge case; no shipped
GUI or CLI control emits custom_agent without agent; no security or data-loss impact).; confidence: confirmed; possible
cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41b_custom_agents_proto_launch/f5`.
