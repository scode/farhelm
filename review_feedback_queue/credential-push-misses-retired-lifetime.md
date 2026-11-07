# A replacement connection can hide credential-push cancellation

Reviewed commit: 49ce8cb76e9f4c841e15122a89264c7c908171dc

## TLDR

A credential push from an ended host connection can keep running if a replacement connection arrives before the push
observes retirement. The new connection then waits behind obsolete transport work, potentially for the SSH operation's
budget. No credential disclosure, wrong-machine delivery or loss is established.

## Details

Suggested bucket: **high**. Confidence: confirmed conditional source race; no runtime reproduction or frequency claim.
Introduced by `cffb06a42563389f4c281c26e50d2057363cac0b` and retained through the reviewed tip.

The delivery entry keeps a reusable watch count in
`crates/farhelm-helm/src/provisioning/credential_delivery.rs:137–153`. Lease drop decrements it at `199–208`; entering a
new lifetime can raise zero to one at `257–270`. Push cancellation tests only the current zero value at `367–377`. Tokio
1.53.2's watch channel retains the latest value, so a push unpolled across `1 -> 0 -> 1` need not observe zero. The old
push and new connect share the serial lock (`306,373`).

This is not fenced by connection setup. A fresh status check, transport dial and hello precede acquiring the new lease
(`manager.rs:3658–3759,3911–3963,4009–4013,4135–4163,4190–4235`; `tensorlake/hosts.rs:82–97`), but nothing forces the
old push to be polled during that interval. SPEC_impl.md, Named credential catalog, `5966` requires final-lease
cancellation before the next transport poll.

If the old push already holds its snapshot, missing zero permits subsequent slot work: Claude settings, Codex
configuration/authentication, and GitHub installation/helper setup/removal (`credential_delivery.rs:412–609`). A queued
push may instead clone the successor's binding after acquiring serialization (`302–309,348–380`); it skips names the new
binding omits and otherwise uses that connection's valid selections. No separate selection violation was found. Observed
cancellation drops the apply future at its next outer select poll; it does not roll back a remote command already sent.

Queued database stamp writes are a separate, preserved ordering boundary. The blocking closure owns the serial guard
through its update (`credential_delivery.rs:748–763`; `crates/farhelm-supervisor/src/db.rs:230–244`), so later delivery
cannot overtake that stamp. Attempt numbers suppress older warning publication (`credential_delivery.rs:157–195`).
Returned failure labels reread the same host entry's current binding (`379–384`); no cross-host attribution or
wrong-destination route was established.

A per-active-lifetime latched cancellation signal could retain counted overlap while making a zero transition
irreversible for its old push. The owning tests' shared-slot cancellation checks do not exercise this missed-zero
schedule. No accepted specification, planned repair, queue item, ledger decision or filter was found to cover it.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g010/g010-lease-zero-coalescing`, with interface handoff
`g012/g012-l1`. The initial possible-highest question was investigated, and the independent delivery consultation agreed
to **high** for the established delay/stale-work consequence only. A demonstrated disclosure or work-loss route would
require reassessing that bucket. Proposed drop: none for the race.
