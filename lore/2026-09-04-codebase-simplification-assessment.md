# Codebase simplification assessment

NOTE: this is an assessment, not a decision or a roadmap. Nothing described here is approved for implementation. It
records a whole-repository review on 2026-09-04 at `cf70aa57`, so the reasoning and the alternatives do not have to be
reconstructed later. Line counts and source references are as of that commit.

The question was whether Farhelm had accumulated architecture, implementation, tests, or specification detail that was
expensive out of proportion to what the tool does, and whether that cost could be removed without losing its core
capabilities or user experience. The short answer: yes, but most of the large code is not ordinary accidental
complexity. It implements deliberately strong promises around durability, host identity, terminal fidelity, version
skew, security, and honest failure reporting. Splitting the large files would make them easier to navigate without
making the system meaningfully smaller.

The largest real simplifications have one of three shapes:

- Stop promising an expensive edge-case behavior, with the contract change stated plainly.
- Remove multiple authorities for the same data or policy.
- Put an irreducible lifecycle under one owner so invalid intermediate states disappear.

## Scale

The repository had about 239,000 lines of Rust, 37,000 lines of browser end-to-end tests, 6,600 lines of scripts, and
about 40,000 words across `SPEC.md` and `SPEC_impl.md`. Five central files alone held about 49,000 lines:

- `crates/farhelm-supervisor/src/service/core.rs`: 15,087
- `crates/farhelm-helm/src/store.rs`: 10,009
- `crates/farhelm-supervisor/src/store.rs`: 9,197
- `crates/farhelm-helm/src/manager.rs`: 7,825
- `crates/farhelm-helm/src/client.rs`: 7,319

Those numbers are symptoms, not the ranking criterion. A file-only split retains every state, guarantee, branch, and
dependency. The recommendations below rank expected net simplification, with contract and implementation risk counted
against raw deletion.

## Top 15 recommendations

### 1. Drop scan-based conversation discovery and keep the launch hook

This is a product decision with very high payoff. The scan subsystem handles vendor-specific record formats, polling,
correlation, ambiguity, durable verdicts, and retry/reverification. `TODO.md:232-240` estimates roughly 6,000 removable
lines across the implementation and its tests.

Exact resume would remain for supported launches whose per-launch hook reports an identity. A custom, opted-out, or
failed-hook launch would remain usable and would truthfully fall back to the existing "identity unavailable" restart
workflow. That is not behavior-preserving: it narrows exact-resume coverage. It is nevertheless the clearest large
deletion because it removes an evidence source instead of rearranging it, and it stops Farhelm tracking vendor storage
formats that are outside its control.

### 2. Remove support for identity-less supervisor connections

Current supervisors report a persistent installation identity, but the helm retains an identity-unverified state, a
live-but-uncacheable session source, REST and UI projections, and dedicated test paths for peers that do not. Requiring
identity during supervisor admission would remove that parallel mode while preserving identity mismatch, explicit
adoption, strict protocol-version refusal, and normal current behavior.

This should happen only after checking the oldest release that remains supported. If that release already reports
identity, this is compatibility cleanup. If it does not, the change is an upgrade-skew break and must be treated as one.

### 3. Keep stable host identity but drop perfect arbitration of millisecond races

`TODO.md:242-257` proposes retaining the behavior that matters day to day: each installation has a stable identity,
address changes do not create a new host, mismatches are visible, adoption is explicit, and two known installations are
never silently merged. What goes away is transactional arbitration when aliases first connect or are adopted in the
same narrow race window.

The estimate in the TODO is about 1,500 implementation lines and 2,000 test lines. This is a product-risk decision, not
a refactor. The smaller design must continue to fail closed when an already-known host reports a different identity;
otherwise it weakens the trust boundary rather than only its adversarial concurrency behavior. It must also state what
happens when two aliases concurrently discover the SAME installation. The current contract presents one host; accepting
a temporary or persistent duplicate row is a separate user-visible cost, not an implementation detail that can be
omitted from the decision.

### 4. Create one authoritative release inventory

Archive names, target triples, installed member names, checksum membership, signing membership, and desktop metadata
are represented separately in Rust, `scripts/install.sh`, cargo-dist configuration, and workflow YAML. The comment at
`crates/farhelm-helm/src/provisioning/assets.rs:1-17` states this duplication directly, and much of that module's test
surface exists only to detect drift among the copies.

Use one checked-in manifest. Consumers should read it directly where practical; deterministic checked-in fragments are
acceptable where POSIX shell or workflow YAML cannot. Keep verification against the actual generated and published
archives. A generator is not evidence that the output is right, and replacing readable copies with an opaque build
step would trade one maintenance problem for another.

### 5. Introduce a serde-only helm HTTP contract crate

Session, profile, host-phase, device-auth, and error shapes are handwritten separately by the helm and UI. Host state in
particular has an internal manager vocabulary, an HTTP projection, and a UI mirror. Put passive request and response
types, stable coarse error codes, and tolerant serde behavior in a small `farhelm-api` crate.

Keep this separate from `farhelm-proto`. Browser JSON and supervisor frames cross different trust and compatibility
boundaries. The shared crate should contain no Axum, reqwest, persistence, routing, or domain behavior; otherwise the
attempt to remove duplicate schemas creates a new shared-behavior layer.

### 6. Share session application operations between REST and agent relay

`crates/farhelm-helm/src/agent_requests.rs` is a second large command surface even though create already trends toward
shared logic in `sessions.rs`. Host availability, launch-snapshot resolution, create idempotency, and lifecycle
eligibility should have one application-level authority that returns typed outcomes.

The browser REST handler and agent relay must remain distinct adapters. They have different authentication, routing,
output, allowed verbs, and failure contracts. Local `farhelm spawn` must also remain supervisor-local and helmless. A
universal `SessionIntent` or generic command framework would erase deliberate authority differences; the useful seam is
the small set of operations whose policy is genuinely identical.

### 7. Give supervisor terminal resources one aggregate owner

Attachment ownership, output-client reaping, sinks, tasks, channels, and teardown quarantine currently live in
parallel registries across `service/core.rs`, `service/terminals.rs`, and `service/teardown.rs`. A `TerminalRuntime` or
lease aggregate should own the resources for one attached terminal and make partially cleaned-up combinations
unrepresentable.

This is worthwhile only if it deletes independent maps and cleanup paths. It must preserve acknowledged cutover,
replay/live ordering, one-viewer takeover, bounded backpressure, pane-mode restoration, server-initiated detach, and
output-process reaping. A generic terminal-backend trait that merely hides those requirements would make the code less
obviously correct without making it simpler.

### 8. Make `SupervisorClient` one explicit mux owner with narrow views

`crates/farhelm-helm/src/client.rs` combines request correlation, terminal channels, uploads, agent upcalls,
cancellation, shutdown, and retirement. Keep one connection task as the sole frame reader and writer, but let owned
mux states manage their maps and cleanup. Callers should receive narrow request, terminal, upload, or upcall views
rather than the entire client surface.

The characterization boundary is cancellation, queue saturation, late replies, channel reuse, upload abort, detach
delivery, and connection-wide failure. Independent connection tasks or an async trait hierarchy would reproduce the
ordering problem in more places, so they are not the target design.

### 9. Concentrate host reconciliation around one exhaustive source decision

Session listing and mutation routing repeatedly choose among connected-with-identity, connected-without-identity,
stale cache, and refused mutation. After identity-less peers are removed, encode the remaining decision once with a
small exhaustive enum and one per-host reconciliation owner or pure reducer.

The behavior to retain is important: live data wins; stale last-known sessions remain visible; unreachable hosts refuse
operations rather than queueing them; identity mismatch freezes until explicit adoption; and retry changes from bounded
attempts to periodic probes. Reject this work if it adds actors, traits, and effects without deleting repeated branches
or independently mutable state.

### 10. Finish concentrating provisioning evidence around the existing frozen plan

The implementation already freezes the displayed actions, paths, unit bytes, and execution choices in one plan, then
consumes that plan once. That should remain the authority rather than being renamed behind a new planner/executor
framework. The remaining concentration opportunity is narrower: `PendingPlan` carries registration and revalidation
facts alongside the executor plan, while payload bytes and their provenance are resolved and staged after confirmation.

Make the confirmed plan refer to one immutable evidence and provenance bundle whose resolution cannot change the
meaning of the action the user approved. Delete any repeated branching that independently chooses add versus update,
identity adoption, payload source, or service action after confirmation. Preserve confirmation-time probing as
revalidation before mutation, signed release verification, local-artifact development mode, privilege boundaries,
atomic per-file installation, and honest partial outcomes. Reject this recommendation if a spike cannot identify
decisions that are currently made twice; the existing frozen plan has already solved most of the problem.

### 11. Consider deleting the invalidation feed in favor of bounded polling

The events feed carries no data. It tells each reader to perform the same REST fetches already used by the fallback,
while adding `/api/events`, subscriber admission, fleet-revision publication, socket retry and health state, special
authentication/CORS handling, and publisher calls throughout manager, store, and profile paths.

Using the existing polling path alone would delete a cross-cutting mechanism. The explicit cost is up to about three
seconds of additional UI staleness and more idle HTTP requests. Measure realistic multi-tab load before choosing it;
if that cost is small, polling is likely the cheaper and easier-to-debug product.

### 12. Replace installer rollback journaling with rerun convergence

`TODO.md:259-274` estimates about 220 production logic lines plus recovery fixtures can go. Keep verified downloads,
same-filesystem staging, atomic replacement of each individual binary, fail-closed output, containment to the install
roots, and safe reruns. The current journal restores the old pair after a catchable failure or on the next run after an
uncatchable interruption. The simpler design instead converges to the new pair when rerun; either design can be mixed in
the interval before that recovery run.

This is a bounded product decision. The installer must never claim success after a partial replacement, and every
interrupted shape needs a convergence test before the journal is removed.

### 13. Reduce the test system by contract dominance, not blanket deduplication

Lifecycle behavior is repeated across unit and service tests, Rust real-stack tests, browser end-to-end tests, desktop
smoke, and installer and provisioning harnesses. Build a traceability table showing which boundary or historical
regression each black-box case uniquely proves, then delete only cases dominated by a cheaper test that observes the
same contract.

Keep pinned tmux, real processes, Chromium and WebKit, desktop process/authentication smoke, POSIX installer execution,
and CentOS-over-SSH. Unit models can explain a state machine but cannot prove those substrates. Separately, move the
hidden fake-agent and test-state commands out of the released `farhelm` binary into a test-support executable; they
currently enter through `crates/farhelm/src/main.rs` and cause e2e-only support code to ship as product surface.

### 14. Rewrite `SPEC_impl.md` as stable architecture rather than accumulated chronology

At 1,789 lines and about 29,000 words, `SPEC_impl.md` mixes current invariants with dated incidents, rejected or
superseded alternatives, validation anecdotes, and build history. Keep current authority, invariants, failure
semantics, and the rationale a future implementation needs. Leave removed history reachable through version control or
put a retrospective extraction in a separate archive that says when it was compiled. Do not backfill `lore/` with
old-looking entries: lore records decisions when they are made and is never maintained after that. New lore remains
appropriate only when an explicitly requested present-day decision needs a record.

This does not remove implementation behavior, so it should not be sold as an architectural reduction. It does reduce
the cost of learning which decisions are current and whether a proposed change violates them. The rewritten document
should be organized by ownership and boundary, with short sections stating what, why, invariants, and failure behavior.

### 15. Prototype a single desktop UI delivery path, but treat it as a security redesign

Having the native webview load the ordinary loopback-served web bundle could remove the second renderer build, custom
asset handler, asset-parity script, cargo-dist shadow packaging, and part of the native HTTP/authentication glue. The
payoff reaches UI, helm routing, authentication, build scripts, smoke tests, and release packaging.

It is not currently a safe refactor. The embedded custom-scheme page is trusted because the native app owns its assets;
loading an ordinary loopback origin raises the question of whether another process holding that port can impersonate
the helm and gain native-app trust. Run a focused WebKit, origin, and server-identity spike first. If equivalent
security cannot be demonstrated, keep the custom-scheme page and its two device sessions, but generate its asset
manifest and packaging metadata from one source.

## Complexity to retain

I would not try to remove the helm/supervisor authority split, strict protocol-version refusal, SSH as the sole remote
transport, tmux ownership of the process and terminal substrate, direct terminal bytes, replay/live cutover, bounded
backpressure, takeover, separate helm and supervisor databases, identity mismatch and explicit adoption, the
loopback/origin/device-authentication layers, or the two-runner signing boundary.

The expensive real-substrate tests should also stay until a replacement seam proves the same thing. In particular,
unit tests cannot prove tmux cutover, WebKit behavior, POSIX installer transactions, SSH provisioning, or published
archive shape.

## Suggested order

The first three items are product decisions, not refactors; each requires a corresponding change to `SPEC.md` and
`SPEC_impl.md` if accepted. The release inventory and HTTP contract crate are the best low-behavior-risk implementation
work. The state-machine items should start as deletion-accounted spikes: count authorities, branches, maps, schemas, and
cleanup paths before the change, then reject any design that adds more interfaces than it removes.

The overall direction is consolidation inward, not flattening boundaries outward. Keep the process, transport,
durability, and trust boundaries explicit. Remove optional promises and duplicate representations around them.
