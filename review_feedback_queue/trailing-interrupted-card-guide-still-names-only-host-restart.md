# Interrupted-card guide still names only a host restart

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The implementation guide describes recovery-card copy that covers one of the two interruption causes.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:3971`.

#4529 changes the Runtime state explanation at 3953–3956 from boot-id detection as the definition of Interrupted to two
distinct routes, including legacy terminal retirement. The same section at 3971–3972 still describes a card that names a
host restart and a pause. The product contract at SPEC.md:3958–3973 names reboot or Farhelm update. The pinned shipped
card's three branches in crates/gui/farhelm-gui/src/session_pane/card.rs:280–286 all name both causes and say the run
ended. The existing card test at card_tests.rs:170–174 asserts the two-cause prefix. The changed classification
paragraph makes this unchanged card text directly relevant. Correct the guide. Bucket: other (documentation).
Confidence: confirmed by pinned source and assertions, no runtime observation. Possible coverage: recovery interface.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l2`.
