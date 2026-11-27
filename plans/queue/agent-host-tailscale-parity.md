# Plan: Enable Tailscale when agents add hosts

Slug: `agent-host-tailscale-parity`

## Goal

Let agents request Tailscale mode when adding a remote SSH host via `farhelm agent host add --tailscale`, display that
setting clearly on the approval card and in the **Add host** dialog's approval summary, thread the user's final
Tailscale choice back in the approval reply, and expose each host's Tailscale flag in `farhelm agent hosts --json`.

### Acceptance criteria

1. `farhelm agent host add <destination> [--alias <name>] [--tailscale]` accepts `--tailscale` and carries it through
   the wire protocol (with `PROTOCOL_VERSION` bumped from 91 to 92 and its pinned version test updated), supervisor, and
   helm.
2. When `--tailscale` is passed, the helm validates the destination format up front using
   `crate::ssh::tailscale::destination_host` before creating an approval card, refusing malformed destinations (such as
   `ssh://` URLs) immediately instead of showing a card guaranteed to fail at probe.
3. The desktop approval card shows `Tailscale host: Yes` when requested (and omits the row when not requested, matching
   optional `Alias`).
4. Clicking **Open add dialog** on the approval card opens **Add host** with **Tailscale host** pre-checked according to
   the request, and the user can still toggle the checkbox before clicking **Add**.
5. The **Add host** dialog's approved-fields summary shows `Tailscale host` (`Yes` or `No`), tracking live checkbox
   toggles (including across setup-refusal reopens).
6. Clicking **Add** sends `tailscale` in `HostAddAnswer` through the existing approval-answer receipt (no new
   wait-for-dialog-completion mechanism), and `AgentReply::HostAddAccepted` returns both the approved `destination` and
   `tailscale` so the CLI confirmation tells the agent when Tailscale was enabled.
7. `AgentHost` includes `tailscale: bool`, serialized in `farhelm agent hosts --json` without bumping discovery JSON
   `schema_version` (keep `8`).
8. All existing host-management safety boundaries stay unchanged: agents still cannot take over a host owned by another
   helm, uninstall Farhelm from a host, delete a Tensorlake sandbox or Podman container host, add standalone persistent
   Tensorlake hosts, change an existing host's destination/Tailscale route/permissions via `host edit`, or bypass setup
   confirmation.
9. `SPEC.md`, `SPEC_impl.md`, and `farhelm agent instructions` prose are updated to match.
10. The covered entry in `TODO.md` is removed and a changelog fragment is added per repository rules.

## Requirement sources

### User request and maintainer decisions

- **Original TODO entry:** "Let agents enable Tailscale when adding a host. Make it possible for agents to support
  directly enabling tailscale when adding a host. Also audit for any other gaps between GUI and agent capabilities when
  adding or removing hosts."
- **Audit scope (settled):** Implement Tailscale on `farhelm agent host add` and expose `tailscale` in
  `farhelm agent hosts --json` (`AgentHost`). Leave deliberate safety boundaries closed (Take over, Uninstall, Delete
  sandbox/container, `host edit` destination/Tailscale/permissions) and leave standalone Tensorlake host add and
  failed-add recovery actions out of scope.
- **Approval reply & dialog summary (settled):** Include `tailscale: bool` in `HostAddAnswer` and
  `AgentReply::HostAddAccepted`, and show `Tailscale host` in the **Add host** dialog's approved-fields summary —
  relying on the existing Add-button approval reply path, with the maintainer's explicit constraint: _"assuming we
  already have the mechanism where we can thread this info through and it isn't adding a whole new 'wait for dialog flow
  to complete' mechanism"_.
- **Up-front validation (settled):** Validate the Tailscale destination format before showing the approval card when
  `--tailscale` is passed.
- **Queue state & review gate (settled):** Planned as `[held]`; per-PR review gate is a single fresh-context Opus 5.5
  agent at high effort with the general review charter.

### Binding repository constraints

- `SPEC.md` ("Topology" Tailscale host option, Host ownership & Take over; "Agent-spawned sessions"
  `farhelm agent host add|edit|remove` and "Agents and environments") and `SPEC_impl.md` ("One route for transport and
  provisioning", "Permission prompts for agent actions", "CLI") remain authoritative; update the sections that describe
  `host add` and `AgentHost`.
- `crates/farhelm-proto/src/lib.rs`: bumping `PROTOCOL_VERSION` is required when an existing message gains a field that
  changes receiver behavior (`AgentVerb::HostAdd.tailscale`), because an older helm ignoring `tailscale: true` would
  silently add the host as ordinary SSH. Do not add cross-version compatibility shims or legacy decode fallbacks.
- `crates/farhelm/src/main.rs` discovery JSON `schema_version` rule: keep `schema_version` at `8` because adding
  `tailscale: bool` to `AgentHost` is purely additive and does not change the meaning of or remove existing fields.
- `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", `.agents/validation.md`,
  `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- `plans/AGENTS.md` and `plans/.agents/executing.md` govern claim checks, record pushes, blocking, and reporting.

### Planner-proposed implementation choices (fresh-context reviewed)

- **Single cohesive PR:** Because `crates/gui/farhelm-gui` is a default workspace member, changing
  `ApprovalAction::HostAdd` and `HostAddAnswer` touches both protocol and GUI compilation together, and the GUI
  production diff is small (~25 lines across 4 files). Ship the change as one cohesive PR rather than stubbing GUI
  fields across two PRs.
- **Validate Tailscale destination once in helm `ParsedAgentVerb::parse`:** `crate::ssh::tailscale::destination_host`
  lives in `crates/farhelm-helm/src/ssh/tailscale.rs`, and `farhelm-supervisor` deliberately does not depend on
  `farhelm-helm`. Keep `farhelm-supervisor`'s `validate_agent_verb` as a doorway bound (`tailscale: _`) and run
  `destination_host` once in `crates/farhelm-helm/src/agent_requests.rs` (`ParsedAgentVerb::parse`) before card
  creation; do not duplicate or move the parser into `farhelm-proto` or `farhelm-supervisor`.
- **Extend `ApprovalAction::HostAdd` in place:** The desktop GUI embeds the helm (always the same build), so add
  `tailscale: bool` directly to `ApprovalAction::HostAdd` rather than adding a parallel variant/tag.
- **Reuse existing helpers cleanly:**
  - Change `crates/gui/farhelm-gui/src/approvals/answers.rs` (`command`, `send`, `start`, `start_unpaused`) from
    `host_add: Option<String>` to `host_add: Option<HostAddAnswer>` rather than threading tuples or an extra `bool`.
  - Reuse `Fact::Words("tailscale host", "yes")` in `crates/gui/farhelm-gui/src/approvals/model.rs` (omitted when
    `false`) and extend `facts<const N: usize>` in `crates/gui/farhelm-gui/src/approvals/host_add.rs`
    (`approved_fields`) to 3 rows (`"Tailscale host"`, `"Yes"` / `"No"`).
  - Capture `let tailscale = self.tailscale;` alongside `destination` and `alias` in `AddHostDialog::add`
    (`crates/gui/farhelm-gui/src/hosts/add.rs`) so the value sent in `HostAddAnswer` is guaranteed to match the value
    probed.
- **Two routine implementation forks for the executor to settle and log as `DECISION`:**
  - Whether `answer_approval` (`crates/farhelm-helm/src/approvals.rs`) also calls `destination_host` on user-edited
    `HostAddAnswer` when `edited.tailscale` is true (4 lines reusing the helm helper to catch bad user edits before
    taking the card), or leaves syntax checks to `/api/hosts/probe` matching `destination_is_usable`. Either way, never
    duplicate `destination_host` into `farhelm-gui` or `farhelm-proto`.
  - Whether `farhelm agent hosts` text output (`crates/farhelm/src/render.rs`) stays JSON-only (matching the settled
    decision and `AgentSession::stale` precedent) or adds a `TAILSCALE` column; either way, never overload the `KIND`
    column (`local`/`ssh`/`tensorlake`/`podman`).

## Implementation outline

One PR (`plan/agent-host-tailscale-parity/01-agent-host-add-tailscale`):

1. **`crates/farhelm-proto`:** Bump `PROTOCOL_VERSION` from 91 to 92 (and rename/update
   `protocol_version_is_pinned_at_91`). Add `tailscale: bool` to `AgentVerb::HostAdd`, `AgentReply::HostAddAccepted`,
   `AgentHost`, `ApprovalAction::HostAdd`, and `HostAddAnswer`.
2. **`crates/farhelm-supervisor`:** Update the `AgentVerb::HostAdd` match arm in `src/service/handlers.rs`
   (`validate_agent_verb`) to bind `tailscale: _`.
3. **`crates/farhelm-helm`:** Validate `crate::ssh::tailscale::destination_host(&destination)` in
   `ParsedAgentVerb::parse` (`src/agent_requests.rs`) when `tailscale` is true; thread `tailscale` through
   `ParsedAgentVerb::HostAdd`, `host_add_for_agent`, `ApprovalAction::HostAdd`, `answer_approval` (`src/approvals.rs`),
   `Verdict::HostAdd`, `Allowed::HostAdd`, `AgentReply::HostAddAccepted`, and `agent_host`
   (`tailscale: view.tailscale`).
4. **`crates/farhelm` CLI:** Add `--tailscale` to `HostCmd::Add` (`src/main.rs`); update `src/agent_client.rs`
   (`HostAddAccepted` destructuring); format `HostAddAccepted` output to mention Tailscale when true; keep discovery
   JSON `schema_version` at `8`; update `src/agent_instructions.rs` prose (the verb usage line is auto-generated from
   clap).
5. **`crates/gui/farhelm-gui` and `crates/farhelm-helm-client`:** Update `approvals/model.rs` (`facts`),
   `approvals/host_add.rs` (`AgentRequest`, `approved_fields`), `approvals/answers.rs` (`Option<HostAddAnswer>`),
   `hosts/add.rs` (`Start::Agent(request)` initializing `self.tailscale = request.tailscale` and capturing `tailscale`
   in `add`), plus affected test fixtures (`farhelm-helm-client/src/collections.rs`, GUI approval/golden tests).
6. **Specs, tests, changelog fragment, TODO removal:** Update `SPEC.md` and `SPEC_impl.md`, add targeted protocol, helm,
   CLI, and GUI unit tests, add a changelog fragment in `releasing/changelog.d/`, and remove the covered entry from
   `TODO.md`.

## Scope reassessment

Before implementing a substantial departure from the outline (for example a new parser, a new wait-for-completion
mechanism, cross-crate validation duplication, or a generalization the outline did not plan), or after repeated
corrective reviews of the same component, run a fresh-context review at the executing session's own model and effort
with this charter, supplying the original request, decisions, outline, current diff, and proposed departure:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

Apply the decision boundary in Unattended fallback below. Give any same-model sub-agents bounded units within the
outline; unapproved new mechanisms are outside their task.

## Checkout and resume protocol

1. **Checkout check:** Run inside the checkout the executing session runs in — the nearest Git worktree or jj workspace
   containing the working directory. Confirm that at least one remote targets `scode/farhelm` (comparing host, owner,
   and repo name). If not, stop before the resume protocol, write nothing, and report the mismatch. All project paths in
   this file are relative to that checkout root.
2. **Working log (`agent-resumeable`):** After claiming the plan per `plans/.agents/executing.md`, resolve the plan's
   record directory with `cargo xtask records path plan/agent-host-tailscale-parity` (`plans/.agents/outside-files.md`,
   "Files outside the repository"). Its `log.md` is the working log (never create a log beside the checkout). Invoke the
   `agent-resumeable` skill with that `log.md` path: if it already exists, read all of it, cross-check against GitHub
   and VCS state, and run the resume check per `plans/.agents/executing.md`, step 7; otherwise start fresh. Push records
   at the cadence required by `plans/.agents/outside-files.md`, "When records are pushed".

## Execution mode

No-workhorse mode (explicit). Do not send any unit of the executing session's own decomposition to a weaker model or a
lower effort than the executing session's own. Do not invoke, load, or follow `scode-galaxy-brain` or
`scode-model-routing` at any point; if `scode-galaxy-brain` is already active when the run starts, this goal file is the
express instruction to stop it.

Same-model sub-agents at the executing session's own model and effort are allowed at the executor's judgment (using
separate worktrees or jj workspaces if concurrent writers might conflict), while the executing session keeps ownership
of the `jjstack` stack and every PR. Launch any model not available as a native sub-agent type (including the per-PR
reviewer) through `scode-harness-shellout`.

The read-only fresh-context sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires" (the resume check,
the cold read of a Blocked question, and the cold read of the report) are always required and exempt from any delegation
restriction. The scope reassessment review runs at the executing session's own model and effort.

## Stack and PR discipline

- Base the stack on `main@origin` (or on this plan's own open `plan/agent-host-tailscale-parity/*` PRs when resuming,
  never on another plan's PRs), set up per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmark names `plan/agent-host-tailscale-parity/<nn>-<short-name>`.
- Keep every PR a draft; never mark a PR ready and never merge.
- Restructure a PR in place instead of stacking a corrective PR on top (including open PRs built by an earlier run).
- Never edit anything under `plans/` in code PRs.
- Before every push of a plan bookmark, follow `plans/.agents/executing.md`, step 9 (`queue status` claim check, push
  records, push bookmark, log pushed commits).
- Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation" (plus
  `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and `plans/.agents/executing.md`,
  step 8 for noting any departure from `## Release note` in the changelog fragment).
- Remove the covered `TODO.md` entry in the last code PR.

## Review gate

Before finishing any PR, delegate an independent review of its changes to a **single fresh-context Opus 5.5 agent at
high effort** (explicit demand for this model and effort, launched natively or through `scode-harness-shellout`; do not
hardcode a launch command). Supply the checkout root, commit range or bookmark, a private findings file in the
executor's scratch directory, and this complete charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address every valid finding and re-verify before finishing the PR.

## Resource watchdog

Right after the resume protocol and before any delegation or heavy build, start the `scode-build-goal` resource watchdog
(resolved via `realpath` from `scripts/watchdog` next to the installed `scode-build-goal` skill's `SKILL.md`; requires
`deno`).

- **Status file:** `farhelm-plan-agent-host-tailscale-parity-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files"), never inside the `plan/<slug>` record directory.
- **Watched paths (`--path`):** the executing checkout root, the home directory, `/tmp`, the parent directory holding
  the status file, the executing session's scratch directory under `/tmp/farhelm-tmp/<id>/`, and any worktree or build
  directory on another filesystem. Interval: 60 seconds, default alert levels (filesystem under 5 GiB free or under 25%
  for filesystems < 20 GiB, or under 10% free while also < 20 GiB; memory/cgroup under 10% available while also < 4
  GiB).
- **Harness mechanics, verification, acting on alerts, and lifecycle:** Follow `scode-build-goal`, "Resource watchdog"
  (Claude Code two-Monitor setup, Codex `skillette-codex-wake-on-exit` setup, or background-sampler + `check` fallback),
  including throwaway `--interval 5` verification before starting the real monitors, stopping any stale prior session's
  monitors on resume, acting immediately on `ALERT`/`WORSE`/`STALE`/`ERROR`, running `watchdog check` before launches
  and at natural boundaries, recording handles/paths in `log.md`, and stopping owned monitors at close
  (`plans/.agents/executing.md`, step 12).

## Decision logging and unattended fallback

- Log consequential design and implementation decisions in `log.md` as they happen with a scannable `DECISION` label and
  the alternatives considered.
- The executing session runs unattended and **never asks the maintainer anything in conversation** — no direct
  questions, no harness question-tool prompt, no approval prompt, no closing "should I...?" or menu of optional next
  steps (`plans/AGENTS.md`, "Unattended means no questions").
- Resolve routine implementation forks within the agreed scope and log them as `DECISION`. Remove unnecessary
  planner-invented machinery when requested behavior and repository constraints still hold.
- When remaining work cannot proceed without a material scope change, weakened guarantee, omitted required behavior, or
  other decision only the maintainer can make, push everything that is consistent and block per
  `plans/.agents/executing.md`, step 10 (`queue block` after a fresh-context cold read). Put things the maintainer
  should know and optional follow-ups into the report rather than stopping or blocking on them.

## Done criterion

The goal is achieved when every PR in the stack exists as a draft PR on GitHub, collectively meets all acceptance
criteria (including removing the covered `TODO.md` entry in the last PR), has passed its Opus 5.5 high review gate, and
satisfies the latest `## Decisions` entry in this plan file (if any). Delivering the report and closing out the claim
happen after the goal is achieved, per `plans/.agents/executing.md`, steps 11–12.

## Release note

No release note needed.
