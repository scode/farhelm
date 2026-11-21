# A held Restart row can settle an Antigravity alert after its trust question is gone

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A refresh during Restart can send a pending trust alert after the Antigravity process that asked has stopped.

## Details

the assigned reader classifies a current trust question as Waiting (agent_kind/screen_reader.rs:480–484). An explicit
Restart confirms the prior agent is stopped before claiming the new generation (service/core.rs:12484–12511,
:12542–12548). It holds the last-listed row with the old status until the replacement publishes (:12742–12777,
:13238–13242). service/listing.rs:114–137 returns the held row as ordinary list data. The helm's periodic refresh is
independent of Restart (farhelm-helm/src/manager.rs:161, :5540–5542). It projects incoming status at :5755–5759 and
:5802–5805 and evaluates it at :5503–5513. ntfy.rs:129–149 settles a pending ten-second spell on a Waiting observation.
A refresh during the hold can qualify if no intervening non-Waiting row cleared the pending spell. The hold itself need
not last ten seconds. core.rs:20307–20315 and :20400–20410 deliberately preserve the display row. Treating that row as a
fresh observation conflicts with SPEC.md:3055–3059 and SPEC_impl.md:5858–5866. Preserve the row for display while
marking it historical for alert admission, or convey Unknown through the existing display-retention path. The relevant
hold and raw alert paths remain at the pinned tip.

Confidence: confirmed by inspection of the reachable list and policy paths within that timing premise.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agScreen/l7`.
