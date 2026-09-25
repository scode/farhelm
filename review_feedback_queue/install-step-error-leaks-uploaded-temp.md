# An install-step error leaks the uploaded temporary

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A flaky connection during host setup or update can leave a large hidden `farhelm-tmp` file in the host's farhelm or bin
directory.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F22 / COR-UPLOADED-TEMP-LEAK-ON-ERROR`, tagged **definite**. Anchors and title: `provisioning/backend.rs:776-783` —
install_uploaded_source leaks the verified remote temporary when its pre-install metadata read or mode repair fails

Remote installs happen in two steps: upload the payload to a hidden temporary next to the destination
(`.<name>.farhelm-tmp-<nonce>`), then, in the install step, verify it and `mv` it into place. The install step,
`install_uploaded_source` (backend.rs:776-783), begins with `self.metadata_on_target(target, destination).await?`, and
its "already installed" path calls `set_target_mode(...)?` and `remove_temporary(...)?`. An error in any of these
returns immediately, before the uploaded temporary is removed. Only a failure of the later verify-and-`mv` command goes
through `cleanup_temporary_failure`. So one transient ssh failure (exit 255) at the start of the install step leaves a
complete binary — tens of megabytes — behind in the host's farhelm or bin directory.

This is separate from the queued item `orphaned-install-temps-on-managed-hosts`, which is about a crash or kill between
steps and assumes install-step errors clean up after themselves; this is an install-step error path that does not.
SPEC_impl.md promises a failed transfer leaves only the installed file. Suggested change: route every error that occurs
after the upload through `cleanup_temporary_failure`.

User-visible consequence: a flaky connection during host setup or update can leave a large hidden `farhelm-tmp` file in
the host's farhelm or bin directory.
