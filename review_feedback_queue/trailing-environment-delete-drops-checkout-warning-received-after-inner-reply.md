# Environment Delete drops a checkout warning received after its inner reply deadline

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

A non-final environment Delete can complete on its host and explain why it left a checkout behind, yet the warning never
reaches the rail if the reply arrives after the inner supervisor deadline.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/sessions/sandbox.rs:245`.

the desktop route owns the late-warning collector past its outer 20-second UI deadline (sessions.rs:4035–4070). The
retained-environment helper wraps the actual reply future in a 240-second timeout (sessions/sandbox.rs:155–166,
:243–255), after which the outcome channel closes without a detail. A sent request has one receiving future
(client.rs:3002–3018, :3900–3918). When the host later answers, client.rs:2247–2259 takes the removed sender and ignores
delivery failure, so an arrived reply reaches neither the collector nor another notice path. The supervisor separately
owns the mutation and bounds only admission there (service/handlers.rs:1365–1392). The non-final path waits for full
teardown and returns the notice (service/handlers.rs:1430–1466, :1503–1516). Teardown includes checkout root and
identity inspection, followed by archive move and sync in a blocking worker (service/teardown.rs:1125–1175, supervisor
store.rs:3809–3830). Source gives no overall four-minute bound on those operations. SPEC_impl.md:269–281 and :6253–6262
allow bounding the inner wait and reporting uncertainty. SPEC.md:2012–2018 and SPEC_impl.md:3783–3789 separately require
retaining a late checkout warning once the host reports it. Keep the request and UI deadlines while giving a later
successful non-final outcome a notice consumer. Bucket: other for the narrow timing and loss of recovery guidance.
Confidence: source confirms that the reply is dropped under this premise, without measuring how long a real filesystem
takes. Possible coverage: agent_requests.rs:18544–18626 releases its reply just after the outer bound, with the inner
receiver still live. Ordinary Replace l7 and agent relay l9 lose warnings at different boundaries. l8 is a missed Trash
epoch after accepted discard. recovery_contract/l1 concerns the unanswered Delete wording.

Independent check of the proposed exclusion. AMBIGUOUS via df619914 sessions/sandbox.rs:153–168 and 243–260,
sessions.rs:4033–4062, client.rs:2245–2261 and 3000–3020, supervisor handlers.rs:1363–1392 and 1428–1457,
SPEC.md:2012–2018, 74b81e51 FILTER.md:24–41. The timeout and one-shot delivery code support loss of a reply arriving
after the inner deadline. A missing warning fits the filter's diagnostic clause. The retained-environment excerpt stops
at Discard teardown, before the notice-producing result and reply, and the timeout comment gives an expectation rather
than establishing rarity for that specific outcome.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/l14`.
