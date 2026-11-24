# CustomAgent::validate imposes no byte length cap on display_name or command

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

Registering a custom agent accepts arbitrarily long display names and executable paths without a size limit. A command
longer than Farhelm's 64 KiB session-create budget can be saved successfully and then fail every launch attempt, and
very large entries are persisted and broadcast in full over the desktop UI channel.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/custom_agents.rs:25`.

In `crates/farhelm-proto/src/custom_agents.rs:25-33`, `CustomAgent::validate` bounds `id` to 32 bytes (line 39), but
checks neither `self.display_name.len()` nor `self.command.len()`. `validate_command` (lines 57-71) likewise has no
length cap. `crates/farhelm-helm/src/custom_agents.rs:62-66` (`save_custom_agent`) delegates validation solely to
`agent.validate()` before writing `spec_json` to SQLite, and `crates/farhelm-helm/src/ui_channel.rs:291-295` broadcasts
the full `Collection::CustomAgents` array on every snapshot/diff. By comparison, every other helm catalog in
`farhelm-proto` enforces explicit byte caps (e.g., `TEMPLATE_NAME_CAP = 128`, `TEMPLATE_FIELDS_CAP = 64 KiB`,
`RECIPE_NAME_CAP = 32`, `BASE_URL_CAP = 2048`, `TOKEN_CAP = 4096`, `validate_host_add_destination` 4096 bytes).

How to verify: Construct a `CustomAgent` with a 100 KiB `command` (e.g. `"/" + &"a".repeat(100_000)`) or 1 MiB
`display_name` and call `CustomAgent::validate()` — it returns `Ok(())`.

Fix: Enforce sensible byte-length caps in `CustomAgent::validate` / `validate_command` (e.g., 128 bytes for
`display_name` matching `TEMPLATE_NAME_CAP`, and 4096 bytes for `command` matching typical POSIX `PATH_MAX` /
`validate_host_add_destination`) and add unit tests in `crates/farhelm-proto/src/custom_agents.rs`.

Reviewer metadata (g41a_custom_agents_proto_types/f2): suggested bucket: other — catalog validation hygiene and
operational edge case. Does not reach `highest`: `/api/custom-agents` is behind the helm's authenticated control-plane
router (`crates/farhelm-helm/src/lib.rs:534-540`), not exposed to untrusted agents or supervisors, and Axum's default 2
MiB JSON body limit caps any single write.; confidence: confirmed (source analysis).; possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`, `crates/farhelm-helm/src/custom_agents.rs:62`
(g45_custom_agents_helm_registry/f3):

In commit 0ae1f6de79be7b712ec33dfd71b3a38edf3b1399 (#4727), `crates/farhelm-helm/src/custom_agents.rs:62-66` delegates
validation to `CustomAgent::validate()` (`crates/farhelm-proto/src/custom_agents.rs:25-33`), which caps `id` at 32 bytes
(`validate_id` at line 39) but imposes no byte-length limit on `display_name` (lines 27-31) or `command` (lines 57-71).
Likewise `crates/farhelm-helm/src/lib.rs:533-540` mounts `/api/custom-agents` and `/api/custom-agents/{id}` without a
route-specific `DefaultBodyLimit` (inheriting Axum's 2 MiB default, unlike `/api/credentials`,
`/api/tensorlake/recipes`, `/api/ntfy-alerts`, `/api/tensorlake`, and `/api/ssh-questions/{id}` at lines 771-889, and
unlike templates which enforce `TEMPLATE_FIELDS_CAP = 64 KiB` in `crates/farhelm-proto/src/launcher.rs:629-634`).
Because `SessionLaunch::field_bytes()` is bounded by SPEC.md's 64 KiB create-field budget, a registration whose
`command` is between ~64 KiB and 2 MiB is accepted by `POST`/`PUT /api/custom-agents` and persisted in SQLite, only to
fail every later launch attempt; similarly, multiple entries near 2 MiB can push `GET /api/custom-agents` over
`ui_channel::MAX_REPLY_BODY` (32 MiB, `crates/farhelm-helm/src/ui_channel.rs:96,942`), causing
`Collection::CustomAgents` to turn `Unavailable`. Note: these routes sit inside `protected` behind
`auth::require_device_session` (`lib.rs:952-958`) and are not exposed to untrusted supervisors or agent sessions, so
this is not a cross-boundary security exploit. How to verify: inspect `crates/farhelm-proto/src/custom_agents.rs:25-71`,
`crates/farhelm-helm/src/custom_agents.rs:62-91`, and `crates/farhelm-helm/src/lib.rs:533-540`. Fix: add explicit length
bounds for `display_name` and `command` in `CustomAgent::validate()` (well within the 64 KiB launch budget) and a tight
`DefaultBodyLimit` on `/api/custom-agents` and `/api/custom-agents/{id}`.

Reviewer metadata (g45_custom_agents_helm_registry/f3): suggested bucket: other — validation inconsistency on
authenticated local/device-session catalog writes; no untrusted-input security boundary is crossed and no user data is
lost.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41a_custom_agents_proto_types/f2`,
`g45_custom_agents_helm_registry/f3`.
