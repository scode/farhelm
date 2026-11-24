# No live UI-channel round-trip contract test covers Collection::CustomAgents mutations

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

Changes to registered custom agents are wired to push over the UI channel, but the helm test suite never exercises a
live create, edit, or delete of a custom agent end-to-end over the UI channel to verify that the served payload projects
and decodes cleanly.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/ui_channel_tests.rs:1433`.

SPEC_impl.md ("What is pushed, and how changes are found", line 444) states that pushed collections are "Enforced by a
round-trip contract test per collection". In commit 0ae1f6de79be7b712ec33dfd71b3a38edf3b1399 (#4727),
`crates/farhelm-helm/src/ui_channel_tests.rs:1433,1474` only added `Collection::CustomAgents` to the empty-list
assertions in `the_snapshot_on_open_holds_every_collection_as_served`, while every other pushed collection
(`Preferences`, `Announcements`, `Templates`, `Notices`, `HostProgress`, `Recipes`, `RecipePlaces`, `SshQuestions`,
`Approvals`) has a live mutation push test in `ui_channel_tests.rs`. Meanwhile
`crates/farhelm-helm-client/src/collections.rs:497-525` (`custom_agents_are_pushed_by_registration_id`) constructs
synthetic `json!({...})` payloads rather than reading `GET /api/custom-agents`, and
`crates/farhelm-helm/src/custom_agents.rs:237-286` tests REST CRUD without a connected UI channel. If
`custom_agents::list`'s JSON envelope key (`"custom_agents"`), `ui_channel.rs:294`
(`split_keyed_array(body, "custom_agents", "id")`), `state.manager.events().bump()` in `custom_agents.rs:177,193`, or
`CustomAgent` wire deserialization drifted, no test in `ui_channel_tests.rs` would catch it. How to verify: search
`Collection::CustomAgents` across `crates/farhelm-helm/src/ui_channel_tests.rs`. Fix: add a test in
`crates/farhelm-helm/src/ui_channel_tests.rs` (mirroring
`a_template_saved_outside_the_channel_reaches_an_idle_connected_ui` at line 1596) that performs
`POST /api/custom-agents`, `PUT /api/custom-agents/{id}`, and `DELETE /api/custom-agents/{id}` against the real router
while a `WsTestClient` is connected and decodes the pushed `Collection::CustomAgents` changes through
`farhelm_proto::custom_agents::CustomAgent` (or `farhelm_helm_client::collections::decode_changes`).

Reviewer metadata (g45_custom_agents_helm_registry/f2): suggested bucket: other — test-coverage gap against
SPEC_impl.md's per-collection round-trip contract rule; the current implementation's wire shape and event bumps match on
inspection, so there is no active runtime bug.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g45_custom_agents_helm_registry/f2`.
