# Retained Mac setup confirmation misses a newly installed desktop app

Reviewed commit: c3bee0e586b954f24a12e665a961eb9e96bf50ca

## TLDR

A remote Mac setup question can remain open while Farhelm.app is installed. Confirming that older question can still
install and start the remote supervisor instead of refusing the desktop-equipped Mac. The source sequence is confirmed;
whether the specification intends to accept this pre-confirmation installation switch remains uncertain.

## Details

Prepare Add host setup while neither standard Farhelm.app location exists, leave the confirmation open, install the app
without launching it, then confirm. `crates/farhelm-helm/src/provisioning/backend.rs:3891` checks app presence during
the initial inspection. The retained Add plan is consumed only at confirmation, in
`crates/farhelm-helm/src/provisioning/service.rs:1087`; the host-scoped run is created afterward, at line 1924.

Confirmation revalidation at `service.rs:2085` checks the registry and probes for a supervisor. With the app installed
but not launched, the absent-supervisor branch at line 2171 returns permission to execute without repeating the app
inspection. The frozen actions in `crates/farhelm-helm/src/provisioning/plan.rs:789` and their dispatch at
`service.rs:2623` then install the program and launchd job.

`SPEC.md`, "Topology" and "Supported host setup", explicitly refuse setup, Update and uninstall when the desktop app is
installed. `SPEC_impl.md`, "Provisioning", says confirmation revalidates the relied-on facts before admitting the run. A
possible exception in `SPEC.md`, "Supported host setup", accepts changing to another best-effort installation while
panel setup or Update is queued or running, and requires no mid-run recheck. An unconfirmed retained plan has no
admitted run yet, so that text does not clearly cover this sequence. Unclear coverage is not grounds to drop it.

Recheck Mac app ownership before admitting the confirmed plan, or explicitly settle whether the exception includes
unconfirmed plans. Discovery-first adoption of an answering desktop supervisor is a different case and should remain
available. Update has a similar missing recheck, but its plan is consumed automatically; Add supplies the clear,
user-controlled confirmation interval described here.

Priority: `other`, based on the established refusal bypass. Confidence: confirmed source mechanism and admission timing;
contract intent remains uncertain. No security, session-loss or material competing-service failure is established.
Earlier reviewers provisionally suggested `high` before narrowing the consequence; their original labels and the
proposed specification-based drop remain in the private history. Proposed drop: not adopted. Possible cover: the exact
queued/running exception above, only if its intended scope includes retained confirmations. No other exact coverage was
found.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`. Original independent identities are `mac-install/mac-install-L1` and
`mac-planning/mac-planning-L2`. Later contract, payload and test interface records preserve the same underlying issue.
