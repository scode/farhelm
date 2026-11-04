# Replace can hide its progress and timeout explanation before the reply

Reviewed commit: f288c370c16f9a9d7d32185054885cb3fbd61161

## TLDR

A slow Replace response can leave the app without a pending indicator while navigation stays disabled. If the request
then reaches its deadline, its explanation is attached to a source row that no longer exists, so that feedback is also
invisible.

## Details

The server deletes the old source before returning the replacement response.
`crates/farhelm-helm/src/sessions.rs:3821–3874` deletes it and awaits `forget_deleted_session`; that method calls
`forget_session` at `3111–3134`. `manager.rs:2193–2246`, particularly `2242–2244`, removes the row and publishes a fleet
event. An authoritative refreshed listing can therefore reach the GUI while the Replace HTTP response is still pending.

`crates/farhelm-ui/src/list/view.rs:1653–1720` reconciles the source's absence and calls `on_removed` without consulting
`row_pending` or the page guard. `lib.rs:2023–2028` clears current selection and unmounts the source's SessionView. The
refreshed `displayed_rows` at `list/view.rs:3517–3544` no longer include that row; `list/rows.rs:457–484` explicitly
removes absent rows even during a pointer-order hold. Its row progress at `list/view.rs:4706–4709` disappears as well.
Both display copies depend on the old source ID, with no persistent fallback for its still-pending map entry.

For a sidebar-started Replace, the list-owned request remains mounted. `row_pending` deliberately survives deletion
reconciliation at `list/view.rs:1685–1686` and is cleared only after the task returns at `2585–2607`. The manual-open
guard refuses a nonempty pending map at `3040–3043`, `nav_locked` stays true at `3471`, and automatic selection waits
for row operations at `3080–3086`. The replacement row can be present while the main area is blank, yet cannot be opened
and has no replacing indicator.

If the 60-second deadline expires, `Refused(reason)` goes into `errors[old_source_id]` at `list/view.rs:2597–2602`, so
the new unknown-outcome explanation has no source row to display it. `SPEC.md`, “Lifecycle operations,” 1464–1468
requires shared progress until the reply and a deadline explanation, stating that the status disappears on the reply.

Keep request-owned progress and result feedback outside source-dependent rows/views until completion. This should
preserve authoritative deletion reconciliation and the operation guard rather than delaying the deletion or enabling
navigation prematurely. A focused verification should delay the response after the source-deletion fleet update and
inspect pending feedback, navigation and the eventual deadline result; no such runtime reproduction is claimed here.

## Confidence and priority

Confidence: confirmed source path under delayed reply, without runtime reproduction. The named production paths remain
unchanged through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

Suggested bucket: `other`, for temporary missing wait/outcome feedback. No lost work, destructive-action or cancellation
bug is established. `could_be_highest: false`. `proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Task ownership and source-removal reconciliation predate the patch; this finding concerns their interaction with the new
progress/deadline display. It is distinct from the announcement-continuity concern `g057-l3`.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, owner `g057`, ID `g057-l6`, latest `g057/findings.jsonl` physical
line13. The latest record confirms pointer-order holds do not retain the row, checks the timeout-result adapter and
follows the server fleet-event ordering.
