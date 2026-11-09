# Mac uninstall test lacks a live retained-host premise

Reviewed commit: c3bee0e586b954f24a12e665a961eb9e96bf50ca

## TLDR

The native Mac test can pass its check that uninstall ends terminal hosts without first establishing that a host was
still running when uninstall began. This weakens the cleanup regression test; it does not establish an uninstall bug.

## Details

`crates/farhelm-helm/src/provisioning/tests/macos_real_tests.rs:743` closes the tab and stops the agent. It checks the
session's ended status, then proceeds to uninstall. The final checks at line 767 compare process identities captured
before Stop. The agent and tab shells and the tab's host are expected to have ended already. The remaining agent host
has no renewed liveness assertion after Stop and before uninstall.

Stop intentionally keeps that host so the user can view the ended agent's terminal; see
`crates/farhelm-supervisor/src/service/handlers.rs:909` and `SPEC.md`, "Lifecycle operations". If a regression also ends
the retained host during Stop, every final process-absence check can still pass even if uninstall never ends a host. The
earlier survival assertion occurs before Stop and cannot establish the later fixture premise.

Assert the original agent host's PID, start identity, command and owned socket after Stop and before uninstall. The
existing helper that requires all original shells to remain alive is unsuitable at that boundary. Then check that
uninstall ends this particular retained host. This follows `.agents/test-authoring.md`'s requirement to establish the
fixture premise before the action that relies on it.

Priority: `other`. Confidence: confirmed missing assertion from source inspection; no runtime failure or shipped cleanup
defect was demonstrated. Proposed drop: none. Possible cover: no matching specification acceptance, planned work, known
bug, queue item or triage decision found. The rare safe-retry filter does not clearly cover an ordinary test-sensitivity
gap.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, originating record `mac-tests/mac-tests-003`. Mac contract and payload reviewers
independently corroborated or preserved this same finding; those interface copies are not additional items.
