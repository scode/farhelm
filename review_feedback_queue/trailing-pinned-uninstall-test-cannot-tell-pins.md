# The pinned-uninstall test cannot tell which pin opened the sign-in connection

Reviewed commit: da215903531243d3bde264dbf50d717d2d222b1d

## TLDR

A test meant to prove that a confirmed Tailscale Uninstall signs in on the connection it just verified would still pass
if the sign-in used the connection prepared at planning time instead. Today's code is correct; the gap is that a future
regression in the sign-in half would ship unnoticed, and then the user's sign-in could open one shared connection while
the removal commands look for another.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t07c-hosts-provisioning` `f6` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept (test strength; the shipped code is correct). No coverage source covers it.

### Reviewer record

Location: `crates/farhelm-helm/src/provisioning.rs:8137`

In `tailscale_uninstall_pins_the_confirmed_peer_or_refuses_peer_drift` (crates/farhelm-helm/src/provisioning.rs at
da21590353), the docstring (around line 7953) says the run uses "one fresh pin for the setup master and every backend
step: the pin confirmation prepared, never the planning pin, which may name another setup master for the same peer." The
commit adds a `relocated` confirmation pin that is `same_peer` as `original` but differs only in its socket directory
(`control_dir`), as the asserts around 8031-8051 establish. The backend-step check (around 8157-8166) compares recorded
`SshAccess` values with the derived `PartialEq`, which includes `control_dir`, so it does tell the pins apart. The
setup-master check (around 8137-8144) instead asserts that each line of `confirmed.options()` appears in the sign-in
receipt. `TailscaleSshAccess::options()` (ssh/tailscale.rs, the method after `prepare_with`) holds only HostName,
HostKeyAlias, the known_hosts file, the fixed checking flags and the proxy. All of those are identical for `relocated`
and `original`, and the control socket path is not among them. So if `SetupSignIn::establish_for_run` (setup_sign_in.rs
around line 433) or the `SignInAccess::Prepared` plumbing regressed to the planning pin, this assertion would still
pass. Verify by reading the two pins' `options()` outputs: they are equal. Fix: also assert that the receipt's
ControlPath (or the socket the stand-in master created) is under `relocated`'s control directory, for example by having
the fixture record `-S`/ControlPath and comparing it to `confirmed.control_socket(SharedConnection::Provisioning)`.

Suggested bucket: other. It is a test-strength gap with no current user-visible bug.

Confidence: confirmed (source-established: options() excludes the control path and the two pins share every option).

proposed_drop: none

possible_cover: none
