# Renaming a custom agent drops its launcher search highlight and refuses the pick as stale

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

If a registered custom agent is renamed while it is highlighted in the launcher's search results, or between drawing a
search result and clicking it, the launcher drops the selection and warns that the choice is no longer available even
though that agent is still present and matches the search.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/search.rs:870`.

In `crates/gui/farhelm-gui/src/launcher/search.rs:215-219`, `Row::CustomAgent { id, label }` documents that "A
registration's ID selects it even after the display name changes." In `6a3db5996c8c9bfbce653c094f8b0ba983f6e1f1`, both
explicit keyboard picks (`search_highlight` at `crates/gui/farhelm-gui/src/launcher/view/search.rs:247-252`) and clicks
(`take_drawn_search_row` at `crates/gui/farhelm-gui/src/launcher/view/search.rs:325-337`) resolve choices through
`SearchChoice::position` (`crates/gui/farhelm-gui/src/launcher/view/search.rs:162-172`), which calls
`same_search_choice` (`crates/gui/farhelm-gui/src/launcher/view/search.rs:870-886`) and refuses any non-match via
`refuse_stale_choice`. When `df0dff4c9b925534b0aae763c325155279a58829` added `Row::CustomAgent { id, label }`, it did
not add an arm to `same_search_choice`, so `Row::CustomAgent` falls into the `_ => a == b` catch-all and compares
`label` as well as `id`. If a user searches by the custom agent's ID (or a shared substring), highlights or clicks the
row, and a store update changes `agent.display_name`, `same_search_choice` returns `false`, dropping the highlight and
refusing the pick with "That choice is no longer available; choose a current result".

How to verify: In a `#[gpui::test]` in `crates/gui/farhelm-gui/src/launcher/search_view_tests.rs`, open the launcher
with a registered custom agent (id `"bot-1"`, display name `"Alpha"`), type `"bot-1"`, press `down` (or capture center
for `click_before_redraw`), push an updated `CustomAgent` with the same `id` and display name `"Beta"`, then press
`enter` (or click). Today it shows `STALE_ROW` instead of selecting `bot-1`.

Fix: Add `(Row::CustomAgent { id: a, .. }, Row::CustomAgent { id: b, .. }) => a == b` to `same_search_choice` at
`crates/gui/farhelm-gui/src/launcher/view/search.rs:871-885` (and consider replacing the `_ => a == b` wildcard with
explicit arms so future `Row` variants with display fields cannot silently regress). Optionally add a
`Row::CustomAgent { id, .. }` store-presence check in `search_row_refusal`
(`crates/gui/farhelm-gui/src/launcher/view/search.rs:389-424`) for parity with `Row::Host` and `Row::Template`.

Reviewer metadata (g24_stale_launcher_choices/f1): suggested bucket: other (narrow UX/correctness bug triggered only
when a custom agent is renamed while highlighted or clicked in launcher search; no security or data-loss consequence).;
confidence: confirmed (source analysis at 1b3fe49af86979aebc2719965682a17f8c6366ac).; possible cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/launcher/view/search.rs:869-884` (g53_custom_agents_gui_launcher_b/f2):

`Row::CustomAgent { id, label }` (`crates/gui/farhelm-gui/src/launcher/search.rs:195-199`) is documented with "A
registration's ID selects it even after the display name changes." However, `same_search_choice` in
`crates/gui/farhelm-gui/src/launcher/view/search.rs:869-884` was not updated for `Row::CustomAgent` and falls through to
`_ => a == b`, which compares both `id` and `label` (`HostText`).

When `SearchChoice` captures a `Row::CustomAgent` on arrow navigation (`search_arrow`, `view/search.rs:283-286`) or row
render (`draw_search_rows`), and a pushed `Collection::CustomAgents` update changes that agent's `display_name` while it
still matches the query (e.g., searching by its stable ID or a shared prefix), `SearchChoice::position`
(`view/search.rs:171-180`) calls `same_search_choice`, gets `false`, and returns `None`. As a result:

- `search_highlight` drops the user's keyboard highlight.
- Pressing Enter (`search_enter`, `view/search.rs:308-311`) or clicking the row (`take_drawn_search_row`,
  `view/search.rs:316-327`) calls `refuse_stale_choice`, showing
  `"That choice is no longer available; choose a current result"` and closing results even though the registration
  exists and appears in the current results.

By contrast, `same_search_choice` matches `Row::Host` solely by `id`
(`(Row::Host { id: a, .. }, Row::Host { id: b, .. }) => a == b`) specifically so label updates do not break identity
matching.

How to verify: Inspect `same_search_choice` at `crates/gui/farhelm-gui/src/launcher/view/search.rs:869-884` and
`Row::CustomAgent` at `crates/gui/farhelm-gui/src/launcher/search.rs:195-199`. In a GPUI test, type a custom agent's ID
in launcher search, press Down/Up to set `typed_choice`, push an updated `CustomAgent` with the same `id` and a new
`display_name`, and press Enter; `refuse_stale_choice` fires instead of selecting the agent.

Fix: Add `(Row::CustomAgent { id: a, .. }, Row::CustomAgent { id: b, .. }) => a == b` to `same_search_choice` in
`crates/gui/farhelm-gui/src/launcher/view/search.rs:869-884`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f2): suggested bucket: other — Minor correctness/UX issue when a
custom agent is renamed while highlighted or clicked in launcher search; no data loss or security impact.; confidence:
confirmed (source analysis of `view/search.rs:160-180`, `view/search.rs:283-327`, `view/search.rs:869-884`, and
`launcher/search.rs:195-199`; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g24_stale_launcher_choices/f1`,
`g53_custom_agents_gui_launcher_b/f2`.
