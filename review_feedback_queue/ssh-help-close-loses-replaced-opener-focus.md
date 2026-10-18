# Closing SSH help can lose focus after a setup offer arrives

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

If Add host finishes checking while SSH help is open, closing help can reveal the setup question without handing
keyboard focus to it. The first key can be consumed recovering focus instead of operating the dialog.

## Details

Add host permits opening help during a pending probe, releases its isolation, and hides the parent
(`crates/farhelm-ui/src/hosts.rs:3964,4067–4071`). A provisionable reply replaces the help button with the setup
confirmation (`3994–4001,4018–4033`). The offer effect (`3954–3957`) attempts focus through
`hosts/settings_dialog.rs:207–215` while that parent is hidden/inert and does not observe help closure.
`ssh_help.rs:26–33` later seeks only `.add-host-ssh-help`, which no longer exists, before reinstalling parent isolation.
Use a surviving safe control or the parent container when the original opener disappeared.

Recovery remains: the parent has `tabindex=-1` (`hosts.rs:3968`); the capture guard focuses it on the next outside key,
consuming that key (`modal_isolation.rs:143–155`). Escape can activate the live Cancel, and subsequent Tab/Option-Tab
can reach the header, named plan region and answers (`provisioning.rs:829–872`). This is not a keyboard trap.

Suggested bucket: **other**. Confidence: **likely**. The missing scripted fallback is confirmed, but setup controls
carry autofocus; shipped-browser behavior when those controls are inserted while hidden and later revealed was not
reproduced. No wrong-target action, security consequence or loss of data is established.

Possible cover: `update-check-replacement-loses-focus.md` covers the desktop version button being replaced by Restart,
not this nested Add host transition. `SPEC_impl.md:1457–1466` requires closing SSH help to restore parent focus.
`SPEC.md:1279–1284` accepts mixed-input races within tens of milliseconds, and `2955–2958` accepts delivered actions on
hidden dialogs; neither matches a slow probe followed by an ordinary close. No matching Planned, BUGS or ledger entry
was found. Timing-filter applicability is not established because the sequence need not be sub-second.

Proposed drop: none. Provenance: persistent Astra/high review, `ui-host-management-2/f2`.
