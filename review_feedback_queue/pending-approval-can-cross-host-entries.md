# A waiting approval can act on a different host

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A Stop approval naming one host can stop a session on another host if session ownership changes while the card waits.
GUI Delete and plain Replace consent can similarly transfer to another host before dispatch. A dishonest attached host
can arrange the repeated session ID; this does not require a random UUID collision.

## Details

The initial route is discarded before approval in `crates/farhelm-helm/src/agent_requests.rs:364–368`.
`sessions.rs:2298–2300` resolves the owner again after approval. The approval guard checks the requesting host's
connection, not the target's original logical host (`agent_requests.rs:1760–1774`; `approvals.rs:414–423`). Removing a
host clears its cache (`store.rs:5817–5858`), and a later refresh can admit the same session ID under another host
(`store.rs:6231–6274`). Collision refusal protects simultaneous claims; it does not bind a waiting approval across
successive owners.

For a concrete adversarial sequence, host A learns honest host B's session ID from fleet discovery. While B is
unregistered, A reports that ID and requests Stop from another session on A. The card names A. B returns, A withdraws
its claim, and B refreshes until the collision clears. A's requesting connection remains live. Allowing the waiting card
then sends Stop to B. A continuously connected third requester can produce the same separation between requester and
target. Rename and Restart have the same missing host binding, with additional title or launch comparisons.

Retain the target HostId when constructing the card and refuse dispatch if current ownership differs. Preserve the
specified behavior for a changed installation behind that same host entry. A fake-peer regression should hold the
approval, transfer the ID through complete refreshes, allow the card, and verify that no operation reaches the new host.

Suggested bucket: **highest**, for wrong-target process termination and a trust-boundary consequence. Confidence:
**confirmed static control flow and ownership transitions**; the complete adversarial sequence has not been runtime
reproduced. Removing and re-adding the same unchanged installation alone need not change the physical target.

Possible cover: SPEC.md's Topology decision at lines 196–205 and the ledger entries
`stop-approval-retain-targets-owner.md`, `restarts-launch-comparison-bind-approval-target-host.md`, and
`rename-approval-transfer-another-installation.md` accept operations following the **same host entry** across retarget
or adoption. They explicitly distinguish replacing that entry. This finding retains only the different-HostId remainder.
SPEC.md:2985–2992 accepts collision refusal, not transfer of pending approval authority. No matching Planned item,
known-bug acceptance, existing queue item, or filter covers this remainder.

Proposed drop: none. Provenance: persistent Astra/high review, `helm-agent-requests-1/f3`, `helm-host-lifecycle-1/f7`,
and `helm-persistence-1/f1`.

## GUI Delete and plain Replace have the same missing owner binding

At reviewed commit `5e1f1cfffe38f41e450ab70d440ca8fabac8be36`, Delete's retained target has id, status and tabs but no
HostId (`crates/farhelm-ui/src/list/shared.rs:28–31`). Row confirmations capture a liveness guard or source Session
(`list/row.rs:2050–2053,2100`), but `list/view.rs:2008–2020,2312–2380` dispatches only id/guard or
id/key/guard/allow_yolo. The API carries no expected HostId (`api.rs:3229–3268,3334–3340`), and dispatch can wait behind
pending configuration saves (`1811–1829`).

Helm routing resolves the current owner (`crates/farhelm-helm/src/sessions.rs:755–780`). GUI Delete has no agent
connection fence (`2761–2805`); plain Replace has no `with.host` comparison (`3253–3294`). A complete host refresh can
withdraw A's claim and admit B as sole owner (`store.rs:6232–6234,6260–6278`). Collision checks and routing rereads
(`sessions.rs:834–895`) reject simultaneous claimants and changes during routing, not a coherent transfer before
routing. A dishonest A can claim a known ID while its real host is unregistered, let the user see A's row, then withdraw
after the real host re-registers under B. A retained confirmation or delayed dispatch may act on B; a live-row warning's
Unconditional liveness guard cannot detect the different host.

Carry the rendered expected HostId through GUI destructive requests and compare it with the routing claim. Keep the
specified semantics for retarget/adoption behind the same host entry. Replace with already sends `Some(bound.host)`
(`crates/farhelm-ui/src/list/create_form.rs:5278–5290`) and the helm compares it to `claim.host`; that protected path is
not part of this added defect. An authoritative GUI refresh removing the ID cancels its pending phase
(`list/view.rs:1495–1505`), so the sequence needs consent or dispatch to survive until the owner changes.

Suggested bucket for this added evidence: **highest**, for a destructive operation on another host entry. Confidence:
**likely**; the missing binding and owner-transfer code paths are confirmed, but end-to-end UI-dispatch timing and an
exploit are not reproduced. This requires an ID moving between entries; honest random UUID collision is not alleged.
Current contested IDs still fail closed.

Possible cover: the existing item's same-entry Topology/ledger analysis remains applicable; it does not clearly accept
another HostId. SPEC.md:2985–2992 and SPEC_impl.md:3230–3242 accept contested-owner refusal, not transfer of pending
consent to a successive owner. The sub-second gesture allowance is not full coverage because ownership may change after
a valid click while dispatch waits. Proposed drop: none; extend the existing item.

Combined provenance after extension: persistent Astra/high review, `helm-agent-requests-1/f3`,
`helm-host-lifecycle-1/f7`, `helm-persistence-1/f1`, and `ui-session-list-1/f2`.
