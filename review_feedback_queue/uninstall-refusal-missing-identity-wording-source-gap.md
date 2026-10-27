# Uninstall refusal may call a missing identity different

Reviewed commit: 79403ca9d554a274c822f2c93c0615936d8473a9

## TLDR

A refusal could describe an unidentified installation as a different installation. A compatible shipped sequence
reaching that missing-identity comparison remains unestablished.

## Details

`crates/farhelm-helm/src/provisioning/uninstall_refusal.rs:129–134` uses categorical wording, while
`provisioning/service.rs:1360–1362` rejects unequal optional identities, including a missing value. The wording/guard
relationship is confirmed; current reachability is a missing source premise. Refusal stays safe and establishes no
harmful action.

Suggested bucket: `other`. Confidence: conditional. Possible cover: the rare-diagnostic filter may apply, but a missing
producer does not establish its rare-trigger prerequisite. Historical identityless-peer decisions do not independently
prove current reachability or a full filter match. Provenance: g027's missing-identity diagnostic candidate and
`a012/f1`. No rendering, provisioning or uninstall reproduction ran.
