# Selection saves omit intermediate session visits

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

Switching through sessions while an earlier selection save is pending can omit an intermediate visit from shared
history. Closing the final session or reopening a client can then select an older session, even when every submitted
request succeeds.

## Details

`crates/farhelm-ui/src/list/view.rs:270–272` records each visit in the optimistic mirror but submits only `Selected(id)`
through the scalar preference queue. `crates/farhelm-ui/src/api.rs:1052–1056` replaces the pending value while a writer
is active; `1063–1067,1323–1329` sends only its newest value after the current PUT settles.

Start with durable history `[D]`, submit A, then visit B and C while A's PUT remains pending. Only A and C reach the
helm. Its transaction can record only received selections (`crates/farhelm-helm/src/store.rs:5055–5066`), leaving
`[C,A,D]` rather than `[C,B,A,D]`. After every issued save succeeds, delete C. The successful preference GET replaces
the richer local mirror (`list/view.rs:2990–3006`), so A can win instead of still-listed B. Reload cannot reconstruct B.
No crash, failed request, upgrade or second client is required. Preserve bounded distinct visit events through the
writer boundary, or explicitly reconcile the history contract with their omission.

The new mirror test directly invokes the pure helper (`api.rs:6525–6562`); the store test awaits each direct update
(`store.rs:11021–11072`); chooser tests start with already formed history. None exercises several selections behind a
pending successful PUT. Correct store atomicity and chooser eligibility cannot recover an event never submitted.

Suggested bucket: **other**, for navigation convenience. Confidence: **confirmed static mechanism; runtime frequency
unmeasured**. No security, wrong-target operation or user-owned-work loss was established. Proposed drop: none.

Possible cover: `SPEC_impl.md:218–226,228–244` explicitly retains latest-wins scalar preferences and permits reads to
overtake ordinary selection saves. This explains final-value ordering but does not expressly reconcile omitted visits
with `SPEC.md:1737–1743`'s most-recent-distinct-visits promise. Preserve that ambiguity for triage. The silent
failed-save exception does not clearly cover successful requests. Concurrent-GUI acceptance is unnecessary to the
trigger. A slow response can last seconds, and refresh does not restore the omitted visit, so the timing/transient
filters do not fully match. No exact Planned, BUGS, current queue or ledger match was found. The existing
upgrade-selection plan record concerns a different mechanism.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; session-history contract, implementation
and landing-report reviewers corroborated one scalar-coalescing defect in later-turn synthesis. No runtime reproduction
was performed.

Original finding IDs: `session-history-contracts-1/f1`, `session-history-contracts-2/f1`,
`session-history-implementation-1/f1`, `session-history-implementation-2/f1`, `session-history-tests-1/f4`.
