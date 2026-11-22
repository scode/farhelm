# An unrecorded setup result is reported as a launch that did not request setup

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A launch that verifies the hook files can later say setup was skipped if only the result write failed.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/farhelm-supervisor/src/service/core.rs:10821-10824,11679-11711,14718-14739,7223-7228,7514-7539,12401-12408;
crates/farhelm-supervisor/src/service/core/host_restart.rs:297-309,367-378;
crates/farhelm-supervisor/src/store.rs:3128-3137,3158-3169,3203-3209; crates/farhelm-proto/src/lib.rs:959-980

Farhelm can tell a user to enable hook setup and start a new session even though it already verified the hook files for
that launch, if the separate setup-result write failed. Reviewed commit 1a53d9d35889d7dde96e31320cc1681dd013737d.
Creation starts with a NULL setup result. Preparation explicitly continues after record_persistent_hook_setup fails and
still publishes Installed from memory when terminal launch succeeds. Create and restart ConfirmRunning update
terminal/outcome and do not retry the setup update. NULL decodes as NotRequested, so a subsequent durable snapshot or
supervisor reload projects a definite no-setup claim from unknown evidence. For a session whose conversation is still
uncaptured, Restart wording (proto lib.rs:1505-1508. GUI session_list/restart.rs:134-140) then says Farhelm did not set
up hooks and points to Settings. SPEC.md:3976-3979 ties that explanation to the actual setup result. Persist the outcome
on confirmation with a retry, or represent unknown without interpreting it as an explicit opt-out. This is separate from
HS-03, which retains a durable Installed result while clearing only the eligibility bit. The reviewer suggested other
for incorrect guidance. The conditional state and explanation are confirmed by source; the failure requires an
unsuccessful setup SQL write followed by usable DB state and missing capture, and no observed incidence is claimed. ; in
supplied inventory.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor/HS-11`.
