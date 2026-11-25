# Trust and hook switches' accessible warnings are untested

Reviewed commit: 101c96a5973fc6967895a335db06a45dd5109638

## TLDR

The fix that lets a screen reader hear the workspace trust risk and the hook removal instructions when it lands on the
switch has no test. Removing it would go unnoticed, and screen-reader users would again hear only the short tooltip at
the switch.

## Details

#4864 associates the existing words with the switches through `Checkbox::accessible_description`
(`crates/gui/farhelm-gui/src/components/checkbox.rs`, ~126-130, wired at ~307-317), called from `agent_trust.rs` `block`
(`.accessible_description(agent.switch_description())`, ~450) and `hook_settings.rs` `row` (~50). The only new test
helper, `accessible_help_contains` (`settings/tests.rs:2278-2288`), passes when any Label node with an element id has a
value containing the text. The help paragraphs that `card::help` turns into Labels satisfy every new assertion on their
own, and none of the assertions looks at the switch node's `description`. Dropping either `.accessible_description(...)`
call, breaking `switch_description()`, or breaking the `describe` flag leaves the tests green.

Fix: in the existing trust and hook tests, find the switch node by `TrustAgent::X.id()` / `agent.id()` and assert its
description contains the risk sentence (and, for hook setup, the removal text).

Suggested bucket: `other` (coverage gap for an accessibility fix). Confidence: confirmed from the test and helper
source; tests not run.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g02-trust-a11y-settings-approvals` record `f3`.
