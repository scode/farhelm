# A retained environment can queue checkout removal without waking Trash

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

After deleting the only session using a checkout in a still-running Farhelm environment, an idle Trash view misses the
automatic cleanup and can delay showing a background failure.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/sessions/sandbox.rs:273`.

this needs no externally planted session or checkout. SPEC.md:1734–1737 and :4756–4762 describe a recipe launch with a
managed checkout. sessions_tests.rs:375–389 and :526–548, :556–599 assert the route dispatches actual managed-checkout
intent to the owned host, including the no-setup home fallback. An approved hostless create or spawn can add a session
using an ordinary directory outside that checkout on the same existing environment (SPEC.md:4723–4736,
agent_requests.rs:2043–2069, :2119–2159, :2162–2207), and the assertions at agent_requests.rs:14694–14725 cover that
route for both providers. An existing source also supports managed-checkout Replace with on its owning environment
(SPEC.md:1725–1732, sessions.rs:4785–4820, :4980–5011). A later GUI session Delete of the checkout's last member with
another session row still present enters the non-sealing Discard branch (service/handlers.rs:1436–1467,
teardown.rs:716–766). teardown.rs:813–859 journals and queues, and the existing teardown test :2525–2563 distinguishes
shared memberships from a queued last-member discard with no archive cue. Checkout listings retain that journal and
annotate worker progress and failures (checkout_trash.rs:433–465, service/trash_remover.rs:206–238). The helm's
sessions/sandbox.rs:273–279 returns the notice/count and forgets the row without advancing the Trash epoch. GUI
trash.rs:253–269 does not reread on a session wake alone. Scope: the documented discard (SPEC.md:1632–1636) authorizes
permanent cleanup; SPEC_impl.md:6236–6238 says “never listed,” while shipped source exposes it as a removal journal, not
a recoverable archive. Report a refresh/notice gap for that shipped listing, consistent with SPEC.md:3214–3222 and
SPEC_impl.md:3865–3878. Keep archived=false and signal accepted removal separately or refresh this host without a cue.
Bucket: other. Confidence: source confirms reachability by composing shipped paths. Existing tests cover the component
contracts separately, with no end-to-end assertion of a preexisting two-session environment discard refreshing an idle
Trash view. No cleanup failure or runtime behavior observed. Possible coverage: cited component fixtures only.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/l8`.
