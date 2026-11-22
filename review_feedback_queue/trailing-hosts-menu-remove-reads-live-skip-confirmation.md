# Hosts menu Remove reads the live skip-confirmation preference instead of what was drawn

Reviewed commit: dfdef7da1653956bb4b48a74281f72d990863bba

## TLDR

Choosing Remove from a host's actions menu checks the live "Remove hosts without asking" setting when the click or Enter
key runs instead of using the state the menu drew. If that setting turns on just before you click Remove—or if Remove
becomes enabled during an unpresented keyboard redraw while the setting is on—Farhelm immediately forgets the SSH host
without showing the confirmation dialog, discarding that host's alias, appearance, checkout folder override,
permissions, and every session's pins, groups, mutes, and read state.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/hosts/menu.rs:514`.

Bucket note: placed in `high`; reviewer surfaced that losing an SSH host's alias, settings, and every session's pins,
groups, mutes, and read marks could border `highest` if treated as non-recreatable user-owned data.

In crates/gui/farhelm-gui/src/hosts/menu.rs:501-521 (touched at line 518 by commit
dfdef7da1653956bb4b48a74281f72d990863bba; pre-existing behavior), HostMenu::render builds the Remove MenuItem with an
on_click handler that captures only host: i64 and calls super::remove::start(host, &mark, window, cx). It neither binds
the drawn skip_host_remove_confirmation preference / HostView snapshot nor gates activation on prompt::ActionFrame /
when_shown. Inside crates/gui/farhelm-gui/src/hosts/remove.rs:136-153, start() reads live listed(host, cx) and live
asks_first(cx) (line 145) at activation time; when asks_first(cx) returns false for an SSH host, it immediately
dispatches DELETE /api/hosts/{id} via submit(&removal, None, cx). This violates SPEC_impl.md, "Questions: the prompt"
(lines 993-997): a control's handler must carry the drawn target and skip preference and admit the snapshot through
prompt::when_shown after presentation so neither a concurrent preference push nor a GPUI dirty-frame keyboard redraw can
dispatch an unconfirmed action the user was not shown. Caveat: Tensorlake and Podman environment hosts
(removal.deletes.is_some()) always enter ask() at remove.rs:145, so machine deletion cannot skip confirmation. Fix
direction: capture a drawn host-removal snapshot (including the host view and asks_first) in HostMenu via
prompt::ActionFrame (as session_list::menu::Actionable does at crates/gui/farhelm-gui/src/session_list/menu.rs:614-643),
admit it via ShownAction::shown() in the Remove handler, and pass that admitted snapshot into remove::start while still
checking live state only to refuse a missing or unavailable host.

Reviewer metadata (g30-gui-refactors-approvals-hosts-prefs/l1): suggested bucket: high (could border highest if
permanently losing per-host settings and all per-session pins, groups, mutes, and seen/read marks on an SSH host is
judged user-owned data that cannot be recreated in a moment; remote supervisor processes and sessions themselves survive
and reappear on re-registration, and environment hosts where removal destroys machines always open the confirmation
dialog regardless of the preference); confidence: confirmed (source and contract analysis); possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g30-gui-refactors-approvals-hosts-prefs/l1`.
