## What this was about

A force quit or crash on macOS could leave recipe tunnels listening on a Tensorlake sandbox after the desktop app had
gone. The maintainer chose to carry those tunnels over the host's existing supervisor connection, after real gateway
measurements showed that a separate SSH forward could retain its local process and held connection.

Recipe tunnels now share the host connection's lifetime and SSH access, including ProxyJump. The supervisor listens only
on the claimed sandbox IPv4 loopback ports; the helm chooses every destination from its own recipe snapshot. Both
directions support half-close, so a service can answer after its caller finishes sending. The Mac app's existing
localhost tunnels gain the same half-close behavior.

## Things you should know

Existing hosts show **Needs update** until their supervisor is updated for the new protocol. Tunnel configuration,
status, backoff and next-connection recipe edits keep their existing behavior. Port 22 remains unavailable, recipes keep
their 32-tunnel limit, and both sides independently cap pending and active tunneled connections at 64. An agent
authenticated only for its session cannot claim listeners, and a sandbox cannot supply a destination to dial.

The real gateway checks used an actual standalone test helm, its production host connection and a held bidirectional
relay. Conservative times measured from before signal dispatch were:

| Action        | Destination connection closed        | Sandbox connection closed | Listener refused | Sandbox connection helper gone |
| ------------- | ------------------------------------ | ------------------------- | ---------------- | ------------------------------ |
| Linux SIGKILL | 144 ms                               | 217 ms                    | 223 ms           | 225 ms                         |
| macOS SIGKILL | 26 ms                                | 97 ms                     | 110 ms           | 111 ms                         |
| Linux SIGSTOP | Checked after ending the frozen helm | 119.8 s                   | 119.9 s          | 120.0 s                        |

The remote supervisor stayed alive throughout. The pause check stopped only the helm, left SSH running, and used the
unchanged 120-second stdin limit. Its timing and observed process state are consistent with the existing watchdog; the
paused helm could not drain the proxy's exit diagnostic, so timing alone does not establish the cause.

Ordinary shared SSH masters could remain after their sessions ended; they carried no tunnel listener or held relay, and
the harness removed its owned masters after measurement. There is no separate tunnel SSH child. After all owned SSH and
inspection activity ended, the sandbox with a 60-second idle timeout was still running at 60.3 seconds and suspended by
65.7 seconds. This establishes idle suspension after disconnection, not while Farhelm stays connected.

One additional small sandbox brought the whole-plan total to four of twenty. It, its attributed snapshot, both throwaway
key registrations and their private files are removed. No live installation was touched. The obsolete SSH test-server
prerequisite, #3072, was closed without merging as the maintainer explicitly requested. The Tensorlake sandboxes guide
now explains shared host access and tunnel lifetime; no preview was started during this unattended incidental
documentation edit.

## Open questions and possible follow-ups

No required decision remains. These checks do not simulate a physical laptop sleeping or every possible network failure;
SIGSTOP supplies the required silent-helm case. The existing separate TODO about stream scheduling remains outside this
change.

## PRs

- [#3135: carry incoming tunnels over supervisor connections](https://github.com/scode/farhelm/pull/3135/changes) —
  draft head `ff84442f`; protocol, listener ownership, admission bounds, shared half-close and early connection
  teardown.
- [#3138: close recipe tunnels with the helm](https://github.com/scode/farhelm/pull/3138/changes) — draft head
  `8c3d37b5`; recipe lifetime, destination selection, lifecycle/status handling, documentation and TODO removal.

## Checks run, reused and skipped

Runtime attempts used the repository's recorder. Selected Rust runs used pinned nextest 0.9.148, four global slots, zero
retries and pinned tmux where required. Selected-out tests are not passes.

- Final integrated recipe, process and lifecycle checks passed twelve cases in `804df358-f5a1-44a0-8bff-8f0c3b79c329`:
  `cargo nextest run -p farhelm-helm --lib -E 'test(tensorlake::tunnels::) | test(lifecycle_holds_end_tunnels_and_resume_reads_the_recipe_again) | test(cancelling_an_unanswered_claim_retires_the_connection)'`.
  These cover real lease drop with another client owner retained, SIGKILL with a held relay, standalone SIGINT/SIGTERM,
  the original recipe's actual ports and destination after a store edit during retry, unknown-port refusal without a
  dial, IPv6, independent large half-closed connections, normal peer loss and typed closed-claim refusal. Complete
  source fingerprint begins `78a6594749b795e0`; output, JUnit and cleanup are complete, with no runtime skip. The final
  revision only adds test-coverage comments and replaces an even-index modulo expression with the equivalent integer
  method requested by Clippy; those changes do not invalidate this run.
- Foundation relay/client/protocol coverage passed 27 cases in `9831f8c2-1a65-432e-9b9b-9bd3f38e7047`:
  `cargo nextest run -p farhelm-supervisor -p farhelm-helm -p farhelm-proto --lib -E 'test(service::tcp_relays::) | test(client::tcp_relays::) | test(tensorlake::tunnels::process_tests::) | test(protocol_version_is_pinned) | test(tcp_relay::)'`.
  This includes authority refusals, atomic claims, independent admission bounds, directional EOF and transfers larger
  than a relay window. It also holds a real connection teardown behind another handler to prove listeners and relays end
  first. Reused from source fingerprint beginning `457ed36c76e05321`; later recipe status changes do not alter these
  pumps or teardown. Linux listener reclaim and wildcard exclusion separately passed three cases in
  `e98ce2f6-2bee-4fe3-b468-7c79b3fc071c`, fingerprint beginning `0174ea39a0e97466`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(linux_reclaims_a_port_while_its_closed_connection_still_reserves_it) | test(tunnel_claim_never_shadows_a_wildcard_listener) | test(other_connections_cannot_take_over_or_consume_pending_sockets)'`.
- The existing desktop TCP integration selection passed five product cases, plus successful fixture-build setup, in
  `426d997d-fc24-442b-b056-dbc009b198ca`: `cargo nextest run -p farhelm --test e2e -E 'test(tcp_relays::)'`. Short HTTP
  responses, IPv6 WebSockets, large bidirectional transfers, target validation and channel reuse/cancellation passed.
  This is reused from the pre-correction recipe tree, fingerprint beginning `fb3dd03bd98dbe9b`: the later changes affect
  recipe refusal/status handling and tests, while the desktop relay route is unchanged.
- The selected recipe, tunnel-status and suspend/resume browser checks passed fourteen cases, seven per engine, in
  `60777f4e-cc70-427c-90c5-97ee43b91c5a`. Command from `e2e/`:
  `npx playwright test 'tensorlake-(tunnels|recipes|lifecycle)\.spec\.ts' -g 'tunnel|a managed recipe is created|the recipe editor refuses|a suspended host has inline Resume|a pending (suspend|resume)'`.
  Chromium and WebKit used three workers, zero retries and three owned real stacks, all cleaned up. The matching CLI and
  Dioxus 0.7.10 web builds passed. Reused source fingerprint begins `f32d53563e965e40`; later changes refine real
  connection-end handling and test helpers, leaving these UI flows and marked fake scenarios unchanged.
- On the dedicated Mac, macOS 26.6.2, arm64, Python 3.15, the local real-supervisor SIGKILL test passed in
  `7b26f419-d865-4eb1-b1c6-97e2b5cdd975`:
  `cargo nextest run -p farhelm-helm --lib -E 'test(=tensorlake::tunnels::process_tests::tunnel_and_listener_end_when_the_owning_helm_exits)'`.
  All ten supervisor relay tests passed in `27447199-4b81-4dc0-88d1-b8fa4d20c85f`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^service::tcp_relays::/)'`. Their complete recorded source
  fingerprint begins `3f4a31c12bd13012`; the frozen Mac binary's SHA-256 begins `37c254e4f43f9c8c`. The later Linux-only
  address-reuse correction does not change Mac behavior. Recipe refusal/status corrections do not change the valid held
  relay or supervisor teardown these checks establish.
- Real-gateway SIGKILL passed from Linux, `09a852c7-691b-4bcc-881a-fcea971e9504`, and from the Mac,
  `f5cc47d5-77f9-47d4-9728-4d13efc42620`; Linux SIGSTOP passed in `7d0cbc65-9077-4f6e-aca4-d510cffd8f13`. These used
  `python3 scripts/record-test-run.py --kind development ... -- deno run -A <private-harness> <owned-config>`. Native
  child waits, process identities, listener ownership, a fresh two-way marker, actual refusal and both held endpoints
  were checked. All primary manifests completed successfully with full output and cleanup. Linux used the frozen product
  tree `835a08cd089ed0d73f1bac3674d51a468e7b0814`, binary hash beginning `c19240a0bafa04ce`; Mac used the separately
  identified build above. These are reused for the final tree: successful claim/dial/pumping and early remote teardown
  are unchanged. Launch-time checkout fingerprints are not substituted for frozen-binary identity.
- Control-plane-only idle observation passed in `4e4226ec-709e-4f25-aa91-cf61421baa78`, using the private idle observer
  after every test SSH/exec channel ended. Cleanup was independently reconciled against the exact sandbox, snapshot
  source ID and key registrations; unrelated resources were left alone.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, scoped dprint,
  `python3 releasing/check-changelog.py format` and diff whitespace checks passed. The isolated pinned-parser
  `python -B scripts/check-test-sleeps.py` inspected 421 delays with no missing rationale. The first final lint attempt
  identified only the equivalent integer-style expression in a test; it passed after correction. Earlier all-target lint
  of all three affected crates and `cargo clippy -p farhelm --bins -- -D warnings` also passed. The shipped-feature
  result is reused for unchanged feature wiring; the final helm lint covers the new common-code branches and tests.
- The docs website build, `bun run build` from `website/`, passed all 36 pages and internal links. Reused after the
  unrelated main changes and code/test corrections: this plan's guide prose, headings and links are unchanged.

Retained failures are not counted as passes. The corrected nine-case Rust run `2e734bb8-472a-4220-a08a-936b1e581463` had
eight passes and one new fixture failure: a probe racing listener teardown received a reset before refusal. The probe
now treats resets as transitional and still requires actual refusal under the original five-second deadline. The exact
test passed in `7ac7c87c-b8eb-4606-afb9-9f772beb8ec6`; the failed aggregate remains failed. Earlier gateway setup
attempts failed before a held marker or measurement action because of an oversized transfer, a reserved-local-row
assumption and an unverified resident-process premise. The fixture was corrected and independently inspected after setup
before the successful proofs. A Mac configuration handoff also timed out before measurement; its transport's zero exit
was not accepted as a remote success.

An earlier Mac build exhausted disk, and an interrupted combined Mac run retained an incomplete cleanup limitation. The
later sequential passes and independent native cleanup audit are separate observations. Initial browser-build commands
selected an unrelated program named dx and failed before compilation; the matching builds used the installed Dioxus CLI
explicitly. These private records remain; no latent-flake claim was made.

The stack was carefully rebased onto main `6703714c`, after reading the full upstream diff. Session-row tooltip changes,
background Git cleanup and release illustrations change different behavior and spec sections. The foundation interdiff
was empty before the separately reviewed typed-closure correction. Integrated focused tests and lint cover the final
sources. Broad workspace/browser batteries, installer, release, native GUI and GitHub CI runs were skipped because the
targeted checks cover the changed contracts. GitHub CI was explicitly excluded by the latest Decision; WebKit browser
coverage is engine-family coverage, not a native Mac GUI claim.

The portable summary over fourteen explicitly selected retained commands completed discovery without invalid inputs:
twelve child commands passed, one failed and one was signaled. It preserves the interrupted Mac command's recorder
cleanup error and incomplete report rather than inventing counts. These are selected observations, not a count of
distinct tests or every setup attempt. Doctests were skipped because the changed docstrings contain no executable
examples.

## Review gate's outcome

Each PR passed the required independent Opus/high and gpt-6.1-sol/high reviews, including bounded correction addenda.
The full test-authoring checklist was supplied. Findings led to bounded admission cleanup, stronger relay/lifecycle
oracles, single refusals, readable diagnostics and correct connection-end status. The proposed ban on retaining another
client reference was rejected: explicit retirement closes the transport independently of reference count, and the real
lease test deliberately keeps such a reference. Fresh own-model scope checks supported the narrow design and fixture
corrections. Both PR texts passed cold reads, and a separate documentation pass covered every touched file.
