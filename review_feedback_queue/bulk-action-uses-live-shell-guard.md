# Marked Stop and Delete use the live preference and row guards

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Marked Stop or Delete can act immediately using newer confirmation settings or liveness than the user saw.

## Details

**highest**. Confirmed by source and contract inspection.

A marked Stop or ordinary Delete can kill live shell or tab processes immediately when the visible control was drawn
before the confirmation preference changed. Source-confirmed, estimated highest. Marking.rs:650–687 consults the drawn
marked keys for availability, but its button captures only action and list. On activation bulk.rs:498–512 fetches each
current row and the live skip preference. At :527–533 the immediate path sends a batch if the current preference skips
or no newly fetched target asks. The same source-supported Settings-save and mouse-frame timing as f3 applies
(settings.rs:273–280, store.rs:49–54, vendor GPUI window.rs:6423–6457). A running shell's ordinary Delete or a Stop with
tabs would require confirmation with the prior preference, so the current preference can directly widen consent. If the
preference was already on, taking the current row also recomputes the sent guard from current liveness and tab presence
(bulk.rs:267–290, :341–355), instead of refusing against the earlier displayed row. This is a structural guard gap in
addition to the established missing-prompt route. SPEC.md:2116–2118,2128–2145 and :2323–2335 requires drawn consent and
per-target guards, and SPEC_impl.md:982–986 explicitly names the drawn skip preference and row. This finding is limited
to immediate dispatch: a batch that asks uses question(action, targets.clone()) and hands the question's targets to
confirm_bulk (bulk.rs:553–567). Environment Delete is always in that lane and has its separate wording issue f4. Single
row/action header use Actionable::drawn to capture row and skip preference (menu.rs:610–656), as shell-contract/f1
independently traces for Stop. Tabs/f5 and f3 share the live-preference failure mode but have a separate initiating
control and do not recompute a row guard. Possible_cover: none in the specs. The initiation defect predates the reviewed
shell copy, but the changed toolbar action applies it to shell sessions.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f12`.
