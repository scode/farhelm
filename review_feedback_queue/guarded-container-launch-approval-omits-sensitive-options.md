# Guarded container launch approval omits sensitive options

Reviewed commit: 91b6dd23cac4856065f360e3969f0a9d98763be1

## TLDR

A guarded launch approval can omit the container image, ports, and token-file choice while authorizing the resulting
launch and credential access.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location: `crates/farhelm-ui/src/approvals.rs:1198-1256 at 91b6dd23cac4856065f360e3969f0a9d98763be1`.

Source assessment (read with the later corrections below) (review record g077/d1, event 21):

Reviewed commit: 91b6dd23cac4856065f360e3969f0a9d98763be1. TLDR: a user can approve what looks like an ordinary agent
launch while the chosen template actually launches a container image with a GitHub token and host-local port access the
card never names. Suggested bucket highest: undisclosed credential/capability grant at the action approval boundary.
Confidence confirmed source-derived supported-input route; no deployed leak, malicious image, credential contents or
exfiltration event is asserted. Code: UI approvals.rs:880-963 routes all actual launch card variants to launch_rows; its
Agent branch :1198-1256 omits selection.container. Only template-write/delete cards call the newly repaired
template_rows container block :1325-1341. proto/session_launch.rs:366-388 display_command shows wrapper/start argv, not
configuration. Compiler proof added in discovery: helm/launches.rs:336-491 preserves container as selection metadata
while generating the same start/resume argv; its test :1190-1211 explicitly asserts both argv vectors are equal with and
without a real image/port/token-file config. yolo_guard.rs:118-143 accepts an exactly recomposed non-YOLO selection with
that container, so this ordinary guarded input is not refused before reaching approval. agent_launch.rs:167-232 applies
a named template into selection.container; agent_requests.rs:2144 resolves it, managed-checkout create :2043-2092
supplies its launch to approve_launch, and :2325-2357 places the snapshot in the card. The post-approval launch actually
uses these choices: supervisor/container.rs:112-128 reads the absolute token file and :315-325 gives its trimmed
contents to Podman exec as GH_TOKEN; supervisor/launch.rs:1232-1245 executes that builder with agent argv. A valid
user-configured container template, a managed checkout on a Podman host, non-YOLO permissions and ordinary
commands-need-approval setting establish the input route. Templates are legitimate catalog inputs; no invented arbitrary
credential-path request is needed. SPEC.md Agent-spawned sessions:3229-3235 requires showing agent choices;
Containerized sessions:3973-3988 defines the hidden port/token grants. Fix: share variable container choice rows between
template and launch cards and explicitly identify ordinary/containerized execution as needed. proposed_drop false;
possible_cover unknown, parent owns matching. Distinct from known g013/f1 fixed-engine-label completeness and g048:l1
Codex provider auth merging. Scope/temporal caveat: this pre-existing sibling launch surface was found while following
the assigned template-container disclosure repair in the same module, not introduced by the repair itself. No changes to
approvals.rs/agent_launch.rs/session_launch.rs through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b; scoped log
reports no later container.rs/launch.rs repair. Source-only: no runtime, browser, network, build or process experiment.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g077/d1`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
