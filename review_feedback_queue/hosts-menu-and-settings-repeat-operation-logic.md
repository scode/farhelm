# Hosts repeats menu and operation logic

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Hosts repeats the code that closes menus, describes lifecycle operations, retains current progress, and saves permanent
answers.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Close the Hosts menu through its existing helper

Remove inlines the weak-menu lookup and close already used by Delete. Call `close` after taking the click’s focus mark,
as Delete does.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/menu.rs:485-494`, :513-518 and :568-574. Haiku out-P3.md §1.
Suggested bucket **other**.

### Share the suspend and resume verbs

Both detail lines match the lifecycle direction to the same infinitive. Share the verb lookup while keeping the
provider-specific subject in each line.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/identity/phase.rs:409-417` and :575-586. Haiku out-P3.md §1.
Suggested bucket **other**.

### Share session-scoped Hosts state

All three store UI state under the current operations-store ID, discard old-session state on change, and notify
observers only when the new value differs. Share the ID check and conditional update, retaining separate globals so each
module keeps its own observers.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/progress.rs:151-180`,
`crates/gui/farhelm-gui/src/hosts/status.rs:425-448`, `crates/gui/farhelm-gui/src/hosts/uninstall.rs:427-449`. Haiku
out-P4.md §1. Suggested bucket **other**.

### Share setup and removal preference admission

Both permanent answers submit a save under the preferences key, clear their line on admission, and show a line for a
busy key or a helm refusal. Share the admission and reply handling with callbacks for each panel’s words.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/setup.rs:888-922`,
`crates/gui/farhelm-gui/src/hosts/remove.rs:429-465`. Haiku out-P4.md §1. Suggested bucket **other**.
