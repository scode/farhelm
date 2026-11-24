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

Additional source record at `1ec7724e3cd0e8353c75af6f9b65842795518e9d`, `crates/gui/farhelm-gui/src/hosts/menu.rs:431`
(g33_remove_host_signin_lock_gui/f2):

Follow-up observation at `1ec7724e3cd0e8353c75af6f9b65842795518e9d`: `super::setup::rerun(&view, ...)` also feeds the
live `view.destination` into `1ec7724e`'s `rerun_command` (`hosts/setup.rs:704-710`) rather than the drawn destination.
The sibling live `asks_first` Remove handler noted in this record is already tracked separately in
`trailing-hosts-menu-remove-reads-live-skip-confirmation.md`.

SPEC_impl.md, "Questions: the prompt" (L995-999) requires controls that ask a question or send a request directly to
bind their handlers to the drawn state (including the skip preference and the parts of state that determine the
command), and notes that GPUI draws a dirty frame before dispatching a key (the same trap `HostMenu` avoids for
`adopt_identity` and `takeover_target` at `crates/gui/farhelm-gui/src/hosts/menu.rs:262-270`). Two handlers in files
touched by `1ec7724e` (pre-existing from `34e6c89f`/`7baa1be2` and `fcbf0def`/`3438488e`, interacting with
`menu.rs:232`, `remove.rs:213`, and `setup.rs:707`) violate this rule:

1. `crates/gui/farhelm-gui/src/hosts/menu.rs:409-438`: `HostMenu::render` computes `which: Rerun` (`Rerun::Setup` vs
   `Rerun::Update`) from `rerun(&view, &offered)`, but its `on_click` handler ignores `which` and calls live
   `current(host, cx)` (`menu.rs:431`, modified in `1ec7724e`) and re-evaluates `rerun(&view, &offered)`
   (`menu.rs:432`). If a failed Update replaces a failed Setup before Enter/click, clicking an item drawn as "Try the
   failed setup again" dispatches `super::update::rerun` (which starts an unconfirmed Update) instead of
   `super::setup::rerun`. Additionally, `super::setup::rerun(&view, ...)` passes the live `view.destination` into
   `1ec7724e`'s `rerun_command` (`hosts/setup.rs:704-710`) rather than the destination the menu header displayed at
   open, even though `SPEC_impl.md:5439-5441` says registered setup names "the destination the desktop showed at the
   click" (note `SPEC.md:453-459` accepts operations following a host entry's destination change, so the `Rerun::Setup`
   -> `Rerun::Update` dispatch flip is the uncovered part).
2. `crates/gui/farhelm-gui/src/hosts/menu.rs:493-513` + `crates/gui/farhelm-gui/src/hosts/remove.rs:136-153`:
   `host-menu-remove`'s click handler does not capture `asks_first` from the drawn frame; `remove::start` reads live
   `asks_first(cx)` (`remove.rs:145`) at click time. If `skip_host_remove_confirmation` flips from `false` to `true`
   after the menu drew, clicking/pressing Enter on Remove skips the confirmation dialog and sends
   `DELETE /api/hosts/{id}` immediately. (`remove::ask` at `remove.rs:207-310`, where `1ec7724e` changed line 213 to
   `offers::current_menu_offers`, properly freezes `Removal` and `enabled` into `Question` once opened;
   Tensorlake/Podman hosts always ask via `removal.deletes.is_some()`.) How to verify: in a GPUI test over `HostMenu`,
   open host 2's menu with a failed Add run (`Rerun::Setup`), push a failed Update run on host 2 without clicking
   outside, and press Enter on `host-menu-rerun` — `POST /api/hosts/2/update` is sent instead of `POST /api/hosts/probe`
   (or refusal). Similarly, open host 2's menu with `skip_host_remove_confirmation: false`, push
   `skip_host_remove_confirmation: true`, and activate `host-menu-remove` — `DELETE /api/hosts/2` is sent without
   opening the confirmation. Fix: freeze `which: Rerun` (and the SSH destination for `Rerun::Setup`) at menu open or
   verify in `on_click` that live `rerun(&view, &offered)` still matches the drawn `which`, and capture `asks_first(cx)`
   when drawing the Remove menu item so `remove::start` uses the drawn skip preference.

Reviewer metadata (g33_remove_host_signin_lock_gui/f2): suggested bucket: other (neither path causes security compromise
or irreversible loss of user data/sessions: Update refuses downgrades and preserves running sessions, and SSH Remove
only forgets the registry entry while keeping supervisor and sessions running per SPEC.md:321-322; provider Delete
always confirms).; confidence: confirmed; possible cover: none (pre-existing handlers outside 1ec7724e's hunk, though
menu.rs:232 `current()`, setup.rs:707 `rerun_command`, and remove.rs:213 `ask()` were modified in 1ec7724e;
SPEC.md:453-459 covers destination changes following a host entry, but not `Rerun::Setup` flipping to `Rerun::Update` or
live `asks_first` skipping confirmation).

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g33_remove_host_signin_lock_gui/f2`.
