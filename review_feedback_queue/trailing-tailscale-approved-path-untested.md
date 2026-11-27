# No helm test checks that a valid Tailscale Allow is accepted and reported to the agent as Tailscale

Reviewed commit: d3c60100482c2ff957aa7e6f9a3594c31c087cd4

## TLDR

The new feature's main path is: an agent asks for a Tailscale host, the user approves it with Tailscale on, and the
agent is told 'with Tailscale'. Nothing on the helm side tests that path. If a later change made the helm always report
Tailscale as off, or refuse every valid Tailscale approval, the tests would still pass. Agents would then be told a
Tailscale host was added as an ordinary SSH host, or users could never approve one.

## Details

The new helm test host_add_tailscale_is_checked_before_the_card_and_answered_by_the_user
(crates/farhelm-helm/src/agent_requests.rs:16988 at the tip) sends only two Allow answers: {destination: ssh://user@dev,
tailscale: true}, expected 400, and {destination: ssh://user@dev, tailscale: false}, expected 204 with a reply of
tailscale false. No test in the helm crate POSTs a host_add Allow with tailscale: true and a valid destination. The only
helm tests answering with tailscale: true use an invalid destination (grep '"tailscale":true' in
crates/farhelm-helm/src/agent_requests.rs and approvals.rs). The other host-add answer tests (approvals.rs, near the
HostAdd cases at 1045 and 1107; agent_requests.rs; e2e agent_listing_real_stack.rs) all answer tailscale: false. The CLI
tests in crates/farhelm/tests/agent_cli.rs (about 3196-3250) print 'with Tailscale' from a mock supervisor's scripted
reply, so they never touch the helm.

As a result, these regressions would pass every test: Verdict::HostAdd or HostAddAccepted hardcoding tailscale: false
(approvals.rs:788-791, agent_requests.rs:3686-3689); answer_approval's Tailscale guard (approvals.rs:779-784) refusing
valid destinations; ParsedAgentVerb::parse dropping a valid Tailscale request. The test's doc says 'the reply is the
agent's only way to learn that the user unticked Tailscale', but only the unticked direction is pinned. SPEC.md
'Agent-spawned sessions' requires the command to say 'when the user approved it as a Tailscale host'.

Fix: in the same test (or a sibling), answer a card with {destination: user@dev, tailscale: true} (and ideally a card
requested without --tailscale, answered with tailscale: true). Assert 204 and HostAddAccepted { tailscale: true }.

Reviewer revision notes: Revision: corrects the tip line numbers (the test is at agent_requests.rs:16988 at the tip, not
16896). I re-checked at the tip: no later commit in the range touches HostAdd code (git log -S 'HostAdd' over crates
after d3c6010048 returns nothing), and still no helm test answers a valid Tailscale Allow. Substance unchanged.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other. This is a test-coverage gap on a new path; the code at the tip is correct
  (approvals.rs:788-791 and agent_requests.rs:3667-3689 carry the answered value through), and per 'Priority buckets'
  harm confined to tests stays out of the higher buckets.
- Confidence: confirmed (source search of the helm tests; no execution).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g14/f1 (finding), group g14 "Agents asking for a Tailscale host on host add: helm, protocol, CLI", findings log
  line 5.
