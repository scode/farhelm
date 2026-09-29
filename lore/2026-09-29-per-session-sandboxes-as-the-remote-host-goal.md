# Per-session sandboxes as the goal for remote hosts, and the first step toward it

NOTE: this is not a decision to build anything beyond the first step below. It records, on 2026-09-29, where the
maintainer wants remote execution to end up, what an assessment of the code at `c0634a7` found about how far the
current architecture is from it, and which piece of work was picked as the first incremental step. Line numbers and
counts are as of that commit. The two earlier vendor assessments,
[`2026-08-30-fly-sprites-as-a-host-kind.md`](2026-08-30-fly-sprites-as-a-host-kind.md) and
[`2026-09-01-tensorlake-sandboxes-as-a-host-kind.md`](2026-09-01-tensorlake-sandboxes-as-a-host-kind.md), are
background; read them first.

## The goal

The end state is on-demand sandboxes, one per session, not sandboxes treated as long-lived hosts. In the maintainer's
words, roughly: "I want to work on this repo, I want to use a Tensorlake sandbox, and I want GitLab credentials given to
that sandbox so that it allows these operations." Farhelm creates the sandbox when the session starts, provisions the
credentials the session is allowed to have, runs the agent there, and destroys the sandbox (and revokes the
credentials) when the session is deleted. Tensorlake and Fly.io Sprites are the two vendors assessed so far; others may
follow. GitLab rather than GitHub for the credentials because GitLab lets scoped token creation be automated, per the
existing Maybe later entry on sandboxed agents with scoped GitLab credentials.

Both earlier lore entries framed a sandbox as a new host kind that the user adds and keeps, with "pause this host" as
the cost lever. That framing is not the goal. Persistent sandboxes might still fall out along the way, but the design
should be judged against the per-session case. The existing local and ssh hosts are not affected by any of this.

## How the code holds up against that goal

The short version: the plumbing is in good shape, and the assumption that a host is a durable machine the user owns
and manages lives mostly in the product model, not in the code structure.

Already behind seams that a new kind would extend rather than rewrite:

- `HostTransport::connect` (`crates/farhelm-helm/src/transport.rs`) returns a type-erased byte pipe, and nothing above
  it knows about ssh except one handshake-EOF hint. A vendor pre-connect step such as Tensorlake's resume would live
  inside a new transport's `connect`.
- The supervisor does not need systemd. `scope.rs` falls back to the process-tree sweep when there is no user manager,
  and `farhelm supervisor run` is a plain foreground command. Only `farhelm helm setup` (the helm's own machine) and
  remote provisioning's plan assume systemd.
- A suspend that freezes processes is mostly harmless to the supervisor: status classification counts samples rather
  than reading the clock, the ticker skips missed intervals, and a cold restart shows up as a boot-id change that marks
  sessions interrupted with resume offered.
- The helm's `HostState` and the UI's phase functions (`phase_label`, `state_detail`, `state_remedy`, and friends in
  `crates/farhelm-ui/src/hosts.rs`) are exhaustive matches, and `IdentityMismatch` is already a "frozen, do not dial"
  state.
- Provisioning plans are data (`ProvisioningAction`), and the executor matches them exhaustively.

Diffused or hard-coded:

- About 8 non-exhaustive kind checks in the helm and about 15 in the UI (`kind == Ssh`, `kind == Local`, or a wildcard
  arm). A new kind silently falls into one branch. In two helm places (`provisioning/service.rs`, the `remote_farhelm`
  rewrite after a probe) that would be an actual bug, and in the UI a new kind gets no edit or remove verbs because
  `manageable` is `kind == Ssh`.
- The schema's `CHECK (kind IN ('local','ssh'))`, a kind/destination shape check, and partial unique indexes per kind,
  plus about 10 SQL string literals spelling the kinds. SQLite cannot alter a CHECK in place, so a new kind means a
  table rebuild.
- Provisioning reaches remote hosts through its own ssh path (`provisioning/backend.rs`), separate from
  `HostTransport`, sharing only the ControlMaster socket directory. A new way of reaching a host has to be taught to
  both.
- Provisioning's service-manager steps are systemd-shaped end to end: a `systemctl --user` probe in `inspect`, and
  unit, daemon-reload, enable, linger, and restart actions in the plan tail.
- The connection actor always dials (re-probe every 45 s, forever), and routing and session creation refuse any host
  that is not connected. There is no wake-on-demand.
- A snapshot clone carries the supervisor's identity and its session rows, so the helm shows the second copy as
  `Duplicate`. "Provision once, start many from a snapshot" needs an identity reset.
- Everything the UI does with hosts is shaped around an ssh destination string and "remove only forgets".

One correction to the Tensorlake entry: payload uploads no longer use sftp. They stream through `cat` over ssh, so the
sprite entry's "no sftp" blocker no longer applies.

## Why per-session sandboxes should still be hosts to the machinery

A Farhelm session is an agent running under a supervisor, which is what provides tmux, terminals, status detection, and
resume. The helm only knows how to talk to supervisors through host rows and their connection actors. The cheapest route
is therefore a host row owned by a session: created with it, hidden from the hosts panel, destroyed with it. The
alternative, driving the agent directly through a vendor's exec API with no supervisor inside the sandbox, throws away
exactly what makes it a Farhelm session, so I do not consider it a real option.

## What the full goal needs beyond the plumbing

None of these are refactors; most are product or security decisions that belong in SPEC.md before code.

- **A sandbox template** becomes the thing a user picks when creating a session, instead of a host: vendor, snapshot or
  image, size, idle timeout, and credential policy. It is long-lived and user-managed, and folder and launch history
  would hang off it instead of off a host row. Whether it is a new concept or an extension of launch profiles is open.
- **Ownership bookkeeping for billed resources.** A helm crash between "create sandbox" and recording it leaves a
  sandbox billing with nothing pointing at it. Record the intent before calling the vendor, tag everything Farhelm
  creates, and reconcile against the vendor's list on helm start. Destroy has to include Tensorlake's suspend
  snapshots, which keep billing after the sandbox is terminated.
- **A multi-step session start** (create, wait, connect, check out, mint credentials, launch) with progress display
  and cleanup of partial failures. The existing fresh-checkout creation flow is a partial precedent.
- **A changed durability contract.** SPEC.md's "Sessions depend on exactly one thing staying up: their host" becomes
  literal: deleting the session deletes the checkout, including the archived working copies delete currently leaves
  behind as a safety net. Suspended means resumable, destroyed means gone, and vendor lifetime caps apply.
- **Credentials.** The helm would hold a GitLab token able to mint narrower ones, the first secret the helm stores, and
  would have to deliver the narrow token into the sandbox and revoke it on destroy.
- **Trust between hosts is a prerequisite, not a follow-up.** SPEC.md's "Local authority and trust between hosts"
  currently accepts, as a temporary exception, that an agent on any attached host can have the helm create or clone
  sessions on other hosts (arbitrary execution on the target) and read every profile's resolved launch bundle, pending
  guardrails in TODO.md's Maybe later bucket. An unsupervised agent in a sandbox, holding credentials and attached to
  the helm, would under today's rules be able to run code on the user's other hosts, including the helm's own machine.
  The same section says future sandbox support needs "a new, explicit isolation contract".
- **Clone-safe identity** moves from optional to required, because installing Farhelm's payloads into every fresh
  sandbox would make session start slow, so sandboxes would start from a prepared snapshot.
- **SPEC.md text** that has to change: cloud-hosted execution as a v1 non-goal, ssh as the one transport integration,
  systemd required for provisioning, and remove only forgetting.

## The incremental path

The first step, and the only one decided now, is a functional no-op refactor of how host kinds are handled: replace
the scattered `kind == Ssh` / `kind == Local` checks and wildcard arms in the helm and the UI with named predicates on
the kind (or exhaustive matches where a predicate does not fit), each named after what its call site actually decides,
and keep the kind's SQL spelling in one place. It is tracked in TODO.md's Near term bucket.

It comes first because every later variant, persistent or per-session, needs it, and because it turns "a new kind
silently takes the wrong branch" into a compile error today. It is deliberately limited to the decisions the code
already makes. The assessment sketched further capabilities (a lifecycle of durable, pausable, or ephemeral, a
Farhelm-owned flag, a service-manager flavor), but none of them has a user yet, and their shape depends on vendor
behavior that is still moving, so they wait for the first kind that needs them.

Likely next steps, in rough order and none of them committed:

- Route provisioning's remote commands through the same per-kind dispatch as `HostTransport`, so a new way of reaching
  a host is wired in once, with a way to close the ssh ControlMaster on demand. Nothing does that today, and pausing a
  billed sandbox needs it.
- Write the design for the full goal, starting with the trust guardrails and the SPEC.md changes above.
- Two cheap experiments to ground it: run a hand-provisioned Tensorlake sandbox as a plain ssh host (the Tensorlake
  entry expects this to work without code changes, and it would confirm the supervisor runs without systemd there),
  and measure session start time from a prepared snapshot, which decides whether starting a session is seconds or
  minutes.
- Then session-owned hosts, the sandbox lifecycle with ownership reconciliation, clone-safe identity, a second
  service-manager flavor for the vendor's process manager, and scoped credentials.
