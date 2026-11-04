# The container-host test lacks an execution-location witness

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The terminal-host container regression can pass when its fake agent runs outside the container, provided the separate
keeper still starts and stops. No production isolation failure is established.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
test-oracle gap; no runtime mutation or failure observed

Source locations: crates/farhelm/tests/e2e/containerized_sessions.rs:1438-1440,1475-1511.

At the pinned tip, containerized_sessions.rs:1438–1440 supplies an executable that only prints readiness and reads
input; :1475–1511 observes output, terminal kind and keeper lifecycle without an agent-side isolation witness.
launch.rs:1232–1251 and container.rs:314–325 currently perform the correct container execution. A container-only
identity or private-file denial witness would distinguish that from local execution. Existing isolation tests use a
different terminal seam. Retain the original source and coverage qualifications; no catastrophic tooling consequence is
asserted.

the fake claude at :1437-1439 only prints CONTAINER HOST READY and waits for input. The test observes that marker
through the terminal host, a running container, TerminalKind::Host, and Stop/Delete outcomes (:1474-1512). A checkout is
deliberately mounted at the same absolute pathname inside/outside (SPEC.md:3737-3738); neither the marker nor the shared
path proves where the fake ran. The production shim first prepares the independent keeper then constructs podman exec
(launch.rs:1183-1194; container.rs:314-325 at cf5188fd16a5cd4dec1e57fc257f0f08037f636d). A regression preserving prepare
but executing the agent locally would still satisfy these observations, including lifecycle cleanup of the separately
prepared keeper. The existing broad isolation test does observe host-only canaries and private credentials
(containerized_sessions.rs:592-619,681-718), but uses SupervisorSeams::default(), which remains Tmux
(SPEC_impl.md:2004-2013), so it does not supply this new host-switch test's missing premise. Have this fake assert a
container-only identity or denial of an owned host-only canary before printing readiness; preserve the existing screen
and lifecycle assertions.

the terminal-host regression does not detect that its fake agent executes outside its container, provided the
independent container keeper still starts and stops. Suggested bucket other; confidence confirmed source-level
test-oracle gap. proposed_drop none; possible_cover none found. The fixture's executable only prints readiness and reads
a line (:1438-1440). All later observations are terminal kind, terminal output, keeper state and lifecycle; none comes
from an agent-side namespace, private-file denial, or container-only identity witness. At the tip the production shim
correctly prepares the keeper then execs the command built by ContainerRuntime (launch.rs:1232-1251;
container.rs:314-325). Those are distinct steps: replacing the final exec with a local execution of the same argv would
preserve the current assertions. This is a competing-mechanism explanation, not a mutated test or production-isolation
allegation. Existing isolation assertions are in a different test using default Tmux seams
(containerized_sessions.rs:592-625,680-719; SPEC_impl.md:2004-2022). SPEC.md:3737-3767 requires in-container execution
and separates the keeper from agent lifetime; SPEC_impl.md:6878-6959 describes that same boundary. Neither accepts this
oracle gap. Initial complete coverage search remains applicable at the same immutable tip: Planned empty
(TODO.md:23-25), no unlanded plan in plans/queue/INDEX.md:1-4, BUGS and queue/ledger searches plus matching
negative-network and template-oracle files found no exact trigger/consequence/scope coverage. Synthesis checked the new
cleanup paths and later Delete change; neither supplies an execution-location witness. Caveats/provenance: g028
later-turn synthesis, source inspection only; no runtime occurrence, shipped security defect, measured flake,
catastrophic tooling harm or user-work loss established. Not potentially highest on the evidence. A container-only
assertion before readiness would strengthen this test while preserving its existing lifecycle checks.

Possible coverage and caveats: none found. Coverage searched at ca9b0a19856dc136f63ad2d14f1076027edce1eb: SPEC.md
(3737-3767), SPEC_impl.md (2004-2057,6878-6959), TODO.md Planned empty (23-25), plans/queue INDEX empty (1-3; remaining
plan files are historical, and inspected account/local-image acceptance text does not address this new terminal-host
oracle), BUGS.md (1-100), queue INDEX (1-1170) and matching negated-isolation-probes-cannot-fail-test.md (1-41) plus
container-template-replacement-test-reuses-old-config.md (1-49), TRIAGE_OUTCOMES keyword search. The queued negative
network oracle is a different assertion and trigger, not coverage. No filter explicitly accepts this ordinary regression
gap.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g028/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is cf5188fd16a5cd4dec1e57fc257f0f08037f636d;
unchanged oracle still present at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (same
file:1438-1440,1475-1498).. This was a source-only review; runtime occurrence and frequency were not measured.
