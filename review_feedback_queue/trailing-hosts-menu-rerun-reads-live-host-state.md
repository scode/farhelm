# Hosts menu Re-run reads live host state instead of the drawn Setup or Update action

Reviewed commit: dfdef7da1653956bb4b48a74281f72d990863bba

## TLDR

In a host's actions menu, clicking Re-run re-reads live host state instead of using the action the menu drew. If a
failed update arrives while the menu is showing Re-run failed setup, clicking it can immediately start an unconfirmed
remote host update instead of probing setup.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/hosts/menu.rs:334`.

In crates/gui/farhelm-gui/src/hosts/menu.rs (pre-existing code in the file touched by commit
dfdef7da1653956bb4b48a74281f72d990863bba): (1) Retry (hosts/menu.rs:310-340) chooses its description/help ("Try the
connection again" vs "Finish the deletion") from super::remedies::deletes_again(&view.state) at render time, but its
on_click handler passes only host: i64 to super::remedies::retry(host, mark, window, cx). Inside
crates/gui/farhelm-gui/src/hosts/remedies.rs:180-186, retry() re-reads live listed(host, cx) and selects
Remedy::DeleteAgain (POST /api/hosts/{id}/retry-delete) whenever live state is HostStateView::Deleting { failure:
Some(_) }. (2) Re-run (hosts/menu.rs:417-447) draws the item from the rendered Rerun variant (Rerun::Setup vs
Rerun::Update), but its on_click handler ignores which and re-reads live current(host, cx) and rerun(&view, &offered)
(lines 439-445), dispatching whichever variant is live (update::rerun starts an unconfirmed remote update; setup::rerun
starts a setup probe). Neither item gates activation on prompt::ActionFrame / when_shown, so GPUI's dirty-frame redraw
before Enter can also activate a newly switched or newly enabled item before presentation. This violates SPEC_impl.md,
"Questions: the prompt" (lines 993-997), and contrasts with Adopt and Take over in the same HostMenu
(hosts/menu.rs:276-278), which freeze their targets at menu open. Fix direction: pass the drawn intent (remedies::retry
receiving drawn deletes_again: bool, and Re-run matching the drawn which: Rerun and refusing if live state no longer
matches) through prompt::ActionFrame / ShownAction::shown().

Reviewer metadata (g30-gui-refactors-approvals-hosts-prefs/l5): suggested bucket: high (could border highest for Retry
if invoking retry-delete on a sealed environment whose earlier deletion failed is treated as unconfirmed machine
destruction when the user clicked "Try the connection again"; note that Deleting { failure: Some(_) } only occurs after
the helm already accepted a seal or explicit Delete per SPEC_impl.md 'Environment Delete and the seal' lines 6351-6370,
so the machine was already slated for deletion, though the user has not confirmed retrying deletion from this menu
item); confidence: confirmed (source and contract analysis); possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/hosts/menu.rs:334-339` (g05-triage-gui-templates-hosts/f1):

Under SPEC_impl.md ("Questions: the prompt": "The same rule starts at the control that asks the question or sends the
request directly. Its handlers carry the drawn session key, row, skip preference and the parts of the environment's
state that determine the command"), menu item handlers must carry the command choice they rendered. Two items in
crates/gui/farhelm-gui/src/hosts/menu.rs violate this: (1) Retry (menu.rs:313-339) renders "Finish the deletion" vs "Try
the connection again" from remedies::deletes_again(&view.state), but on_click passes only host to remedies::retry(host,
mark, window, cx), which re-reads live listed(host, cx) at crates/gui/farhelm-gui/src/hosts/remedies.rs:180-186 to
choose Remedy::DeleteAgain (POST /api/hosts/{id}/retry-delete) vs Remedy::Retry (POST /api/hosts/{id}/retry). (2) Re-run
(menu.rs:417-446) renders which (Rerun::Setup vs Rerun::Update), but on_click ignores which and re-reads current(host,
cx) + rerun(&view, &offered) to dispatch setup::rerun vs update::rerun. By contrast, Lifecycle (menu.rs:343-362) and
Adopt/Take over (menu.rs:366-416) properly bind their drawn action/target. Fix direction: capture the drawn
deletes_again bool (or Remedy::Retry / Remedy::DeleteAgain) in Retry's on_click and pass it to remedies::retry (refusing
if live state no longer matches), and capture the drawn Rerun variant in Re-run's on_click (refusing if live state no
longer offers that same Rerun variant).

Reviewer metadata (g05-triage-gui-templates-hosts/f1): suggested bucket: other (could NOT be highest: retry-delete is
refused by the helm unless the host is already sealed with a failed deletion per SPEC_impl.md "Environment Delete and
the seal", and Re-run opens setup confirmation or starts Update rather than destroying unsealed user data); confidence:
confirmed; possible cover: none.

Scope split: the sibling Retry live-state handler in `hosts/menu.rs:310-340` is already tracked in
`stale-host-retry-can-continue-sandbox-delete.md` (extended in this review with the `66ad3aaa` references and
`g30/l5`+`g05/f1` notes). This item records the uncovered Re-run handler at `hosts/menu.rs:417-447`.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g30-gui-refactors-approvals-hosts-prefs/l5`, `g05-triage-gui-templates-hosts/f1`.
