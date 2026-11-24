# Custom agents whose ID matches a model family word are hidden in unlabelled search when the model catalog is loading or unavailable

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If you register a custom agent with an ID that happens to be a model family word (such as `auto`, `sol`, `flash`,
`spark`, or `luna`), typing that ID in the launcher's unlabelled search shows "Loading models…" while the model catalog
is loading and "No model matches" if the model catalog failed to load, hiding the registered custom agent.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/search.rs:552`.

- `farhelm_proto::custom_agents::validate_id` (`crates/farhelm-proto/src/custom_agents.rs:37-54`) permits IDs matching
  `CATALOG_FAMILIES` (`crates/farhelm-proto/src/launcher.rs:55-71`, e.g. `auto`, `sol`, `flash`, `spark`, `luna`,
  `terra`, `astra`, `neon`, `argon`, `composer`, `fable`, `glm`, `glm-flash`, `hydrogen`).
- In `crates/gui/farhelm-gui/src/launcher/search.rs:529-561`, `family_word` is true for those words, while
  `colliding_harness` (lines 532-538) checks only `HARNESS_SEARCH_ORDER` (built-in harnesses) and does not check
  `inputs.custom_agents`.
- Consequently, when `scope == Scope::All` and `inputs.catalog.is_none()`, line 552
  (`if any(Scope::Model) && inputs.catalog.is_none() && preferred_harness.is_none() && family_word`) returns early with
  empty `rows` and `empty_message(Scope::Model, ...)` before `inputs.custom_agents` is searched at lines 591-603.
- When `inputs.catalog` is pending, the user cannot pick their already-loaded custom agent (unlike built-in `grok`);
  when the catalog read failed (`catalog: None, catalog_pending: false`), the custom agent is completely unfindable by
  unlabelled search (`"No model matches; enter a custom model ID in the model field"`).
- How to verify: In `search_tests.rs`, set `draft.catalog = None`, `draft.catalog_pending = false` (or `true`), add a
  `CustomAgent` with `id: "auto"` (or `"sol"`), and call `draft.search("auto")`; observe `rows` is empty.
- Fix: Before taking the early catalog-wait return at `search.rs:552` (when `scope == Scope::All`), check whether
  `inputs.custom_agents` contains a registration with `agent.id.eq_ignore_ascii_case(value)` (or display-name match),
  and do not return early when an exact custom-agent ID match is already present.

Reviewer metadata (g52_custom_agents_gui_launcher_a/f5): suggested bucket: other (narrow edge case requiring both a
custom agent ID matching a model-family word and a pending/failed model catalog read).; confidence: confirmed; possible
cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g52_custom_agents_gui_launcher_a/f5`.
