# Sandbox sessions: a session in a Tensorlake sandbox of its own

Written against main at c277fa78 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

This is the third of three chained plans. It runs after `prepared-snapshots.md`, which itself runs after
`recipe-bases.md`.

- `recipe-bases.md` gives every recipe a base that Farhelm always builds on (packages, Claude Code, Codex).
- `prepared-snapshots.md` gives every recipe a Farhelm-managed prepared snapshot. That snapshot has the supervisor
  installed and no identity, expires after 24 h or on a Farhelm update, and is prepared lazily through an "ensure
  prepared" step. New Tensorlake hosts already start from it.

Read both plans' landed code before starting.

The TODO.md entry (bucket "Near term", subsection "Tensorlake"), verbatim:

> - **Phase 2: a sandbox per session.** A session launch names a recipe instead of a host; Farhelm creates a sandbox for
>   that session from the recipe's prepared snapshot, runs the session there, and deletes the sandbox with the session
>   (one sandbox per session, mapped 1:1). Phase 1 (Tensorlake hosts from named recipes, prepared snapshots and Rebuild
>   snapshot, suspend/resume, delete with snapshots, recipe tunnels, the list of sandboxes not on this helm) is the
>   base; Rebuild snapshot was put into phase 1 because phase 2 needs exactly that path. Design records:
>   `lore/2026-11-03-tensorlake-sandbox-hosts-design.md` ("Phase 2, as far as it was thought through", plus the measured
>   platform facts) and `lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`. […]

Read the whole entry in TODO.md. Its sub-bullets list what phase 2 needs. Per-repository snapshots, live resize, shared
volumes and warm forks are **out of scope** here. The trust questions are answered by refusing agent operations outright
(below).

The maintainer's words while planning (2026-11-07 and 2026-11-08):

> i want to use our planning system to get our tensorlake support to the point where I can create a session from a
> template attached to a tensorlake recipe or recipe-like concept. right now we have support for "treat a tensorlake
> sandbox as a host". That will stay. But I want "use a disposeable tensorlake sandbox for a given session" (a named
> sandbox so the user is incontrol ofwhen data goes away).
>
> to start with suspend/resume would be entirely explicit by the user (buttons on top of the session, and in the menu
> for sessions), with a default timeout set so that tensorlake shuts it down eventually if the helm goes away.

> trust - for now lets scope down and not allow it at all, no agent operations against farhelm. near term todo to
> address it.

> for claude setup - for the first iteration lets not worry about it, the experience will only be smooth with a load
> balance tunnel not with oauth credentials.

> yes. ut note: suspend ALONE must remain. that's a key property and awesome thing - you can actually suspend it w/o the
> processes dying etc.

mise was discussed and dropped: "no mise at all in the entire plan." Nothing here installs or runs mise.

### The model

- A **sandbox session** is an ordinary Farhelm session whose host is a Tensorlake sandbox created for that session
  alone, from a recipe's prepared snapshot.
  - The host is a hidden, **session-owned host row**: a Tensorlake host whose lifecycle the session drives. It never
    appears in the hosts panel, the host rail, the host filter, or the agent-facing host listing. Its connection, status
    check, suspend, resume, credential delivery and tunnels reuse the Tensorlake host machinery, as the 2026-09-29 lore
    entry argues.
  - It is a **named** sandbox, so suspending keeps its data. Only deleting the session removes it.
- Sandbox names use a new reserved prefix, `farhelm-sess-<helm tag>-…`, chosen before creation like `farhelm-host-`
  names. "Sandboxes not on this helm" recognizes the prefix and lists a sandbox with it that no row on this helm owns.
  It also lists a sandbox that a session-owned row on this helm owns but no session uses: a launch the helm stopped in
  the middle, or one whose cleanup Tensorlake refused. That row is hidden everywhere else, so without this the sandbox
  would bill with nothing the user can see or delete. Its Delete there terminates the sandbox, deletes its snapshots and
  forgets the row. Nothing cleans up automatically at helm start, as for every other Tensorlake leftover.
- The sandbox is created with a **30-minute idle timeout** and automatic resume off.
  - Tensorlake counts an open ssh channel as activity, and the helm's connection keeps one open with a 30-second
    heartbeat. So while the helm is connected the timer never runs.
  - When the helm goes away, the supervisor connection's two-minute silence limit ends the remote side, and Tensorlake
    suspends the sandbox 30 minutes later. Farhelm adds no timer of its own.
  - This is a fixed value, stated in help text, not a setting.
- **Suspend and resume are explicit.** Farhelm never suspends or resumes a sandbox session on its own, matching the
  Tensorlake host rule.
  - When the helm starts and finds a sandbox session's sandbox running, it connects as usual.
  - When it finds it suspended (by the user, the idle timeout, or the `tl` CLI), it shows the session suspended and
    waits for Resume.
- **Delete** terminates the sandbox and deletes its snapshots, then removes the session and its host row: the same
  removal order and proof as deleting a Tensorlake host (`tensorlake/hosts.rs`, `delete_sandbox`).
- **Clone** launches a new sandbox session from the same recipe, with the same settings. It never copies the source's
  disk.
- **Restart, Restart with, Replace and Replace with** stay in the same sandbox.
- **Agents in a sandbox session can't ask Farhelm to act.**
  - The helm refuses every agent request whose origin host is session-owned: `farhelm spawn` (which arrives as an agent
    create), `farhelm agent …` create, clone, lifecycle verbs, template verbs and host verbs alike. The single place is
    `crates/farhelm-helm/src/agent_requests.rs`, `HelmAgentRequests::handle`, right after the verb is parsed, keyed on
    `origin.host`.
  - The agent gets a plain refusal: "Agents in sandbox sessions can't ask Farhelm to act yet."
  - The session also gets a quiet notification on its bell (the session notification list in SPEC.md's Status section),
    saying the agent tried and what it asked for, at most once per launch of the agent so a looping agent cannot flood
    the bell. This is a new notification kind; update SPEC.md's statement about which notifications exist.
  - Approval cards never appear for these requests.
  - How the bell learns of it: the refusal happens in the helm, but session notifications are the supervisor's. The
    supervisor's agent relay (`crates/farhelm-supervisor/src/service/agent_relay.rs`) sees the refusal come back,
    carried as the existing unauthorized error kind or a distinct one, and records the notification. The notification
    store already allows one entry per kind per launch (`service/notifications.rs`), so use one new kind and let that
    rule do the deduplication: at most one such notification per launch of the agent, naming the first request that was
    refused.
- **Agent requests aimed at a sandbox session from elsewhere.** An agent on another host can today ask to act on any
  session, through an approval card. For a sandbox session, the helm refuses an agent's Delete, because only the user
  deletes a sandbox (SPEC.md's Tensorlake host rule). It also refuses an agent's Clone, because that would create a
  billed sandbox without the creation confirmation, and agents can't request sandboxes yet (TODO.md's "Agent-requested
  Tensorlake hosts"). Other verbs aimed at a sandbox session (Stop, Restart, Replace, rename and the like) keep today's
  approval flow. This was a planner decision made after the maintainer's answers, to keep the "no agent operations"
  decision from leaking through other hosts; the maintainer sees it in the planning PR.
- **Billing and creation confirmation.** The first launch to a sandbox asks, unless the existing **Create Tensorlake
  sandboxes without asking** switch is on.
- **Host permissions.** A session-owned host takes the defaults new Tensorlake hosts get today, including **Start YOLO
  sessions without asking**. It is hidden, so nobody could change them anyway.

### The UI that was agreed (mocked up on the planning page)

The mockups reused today's Steel components; reuse their classes and tokens.

**Launcher (New session)**

- The host picker's listbox keeps hosts first. Below them, under a small group heading "New sandbox for this session",
  it lists every recipe:
  - each with a new **cube** host mark and its size as secondary text ("4 CPU · 8 GB");
  - a recipe with no built snapshot is disabled, its secondary text giving the reason ("No snapshot yet"). The same
    applies when there is no Tensorlake API key, or when Farhelm's SSH key is not registered.

  The closed picker reads "New sandbox · dev-box" with the cube mark.

  The cube mark is new geometry in `crates/farhelm-ui/src/icons.rs`; read `docs/harness-marks.md` first if it applies to
  host marks too. Its outline in a 16-unit box:

  ```
  M8 1.5 14 4.75v6.5L8 14.5 2 11.25v-6.5Z M2 4.75 8 8l6-3.25 M8 8v6.5
  ```

  Stroke 1.3, round joins, like the other host marks.
- Under the Host row, an explanatory note in the quiet tone (not the warning amber):

  > A new Tensorlake sandbox from recipe dev-box, just for this session. It bills while it runs: use Suspend on the
  > session to stop paying for CPU and memory. Deleting the session deletes the sandbox and its files.
- Destinations:
  - **Managed checkout** works as on any host. Clones of private repositories use the recipe's GitHub token, which is
    delivered on connect.
  - **Folder** is fixed to the sandbox's home: no Browse and no recent folders, because the sandbox does not exist yet.
- The footer's launch context reads, for example, "Claude · new dev-box sandbox · scode/farhelm".
- Before creating, a confirmation in the style of the Tensorlake host dialogs (cube tile):
  - Title: "Create a sandbox for this session?"
  - Summary: "Recipe dev-box · 4 CPU · 8 GB · 40 GB"
  - Body: "Farhelm creates a Tensorlake sandbox that only this session uses. It bills while it runs (Tensorlake
    pricing). Suspend it from the session when you're not using it. If Farhelm loses its connection to it for 30
    minutes, Tensorlake suspends it. Deleting the session deletes the sandbox and its files."
  - Buttons: Cancel, and **Create and launch** as the primary.

  The **Create Tensorlake sandboxes without asking** switch skips it. Update that switch's help text to cover sessions.
- **No placeholder session while the sandbox is created.** The existing launch progress (the composer footer, or the
  floating pill for quick launches and templates) names each step with its elapsed time:
  - "Preparing dev-box for sessions…". Only when needed, and worded to say this happens only on the first launch and
    then at most once every 24 hours or after Farhelm updates. For example, a second line or a parenthetical: "(first
    launch; then at most once a day)".
  - "Creating a dev-box sandbox…"
  - "Starting Farhelm in the sandbox…"
  - then the ordinary "Launching "<title>" on dev-box sandbox…".

  On failure Farhelm deletes the half-made sandbox and its host row, and the composer shows the reason as a refusal
  ("Not launched. …"). The managed checkout's clone then runs in the session's own terminal, as it does today on any
  host.
- **Templates and recent setups.**
  - A template's Host field can name a recipe target. In the template editor's Host choice, recipes appear after hosts
    as "New sandbox · dev-box", grouped as "New sandbox for each session" when the control is a native select.
  - The template stores the recipe by name; recipe names never change. If the recipe is deleted, the field is
    inapplicable, the same rule as a host that no longer exists.
  - "Save as template" from a sandbox launch saves the recipe target.
  - A recent setup or Clone prefill from a sandbox session targets the recipe (a new sandbox), never the old session's
    host.

**Session view**

- The toolbar gains **Suspend**: its own button just left of Stop, with a new pause glyph (two rounded bars). While the
  session is suspended, the button becomes **Resume**: the toolbar's one filled primary control, with a play glyph.
- While suspended:
  - **Stop, Restart (and its menu) and Replace (and its menu) are disabled.**
  - **Clone and Delete work.**
  - The terminal area shows the existing session-unavailable overlay card with sandbox-session wording:
    - Heading: "Session suspended", with the pause glyph instead of the spinner.
    - Body: "This session's sandbox is suspended. Its processes are frozen exactly as they were, including the agent's
      running command, and they continue when you resume it. While suspended, only snapshot storage bills."
    - A **Resume** primary button.
    - A detail line, either "Suspended from Farhelm at <time> · sandbox <name>" or, for a suspend Farhelm did not ask
      for, today's outside-Farhelm wording adapted to sessions, for example "Suspended outside Farhelm, noticed at
      <time>. Tensorlake suspends a session's sandbox after 30 minutes without a connection from Farhelm." Farhelm
      cannot tell the idle timeout from a suspend made with the `tl` CLI, so the wording must not claim which it was.

    While the sandbox changes state, the card reuses today's suspending and resuming wording and spinner.
- Suspend follows the existing Tensorlake host Suspend rule exactly: it asks first in the same cases the host Suspend
  does (an agent working, or its status unknown or stale; check the code), with the dialog's wording adapted to the
  session. A planner proposal for consistency; log a DECISION if the code suggests otherwise.
- The bottom status line names the host "dev-box sandbox" with the cube mark. While suspended, a quiet "Suspended" word
  sits where the age is.

**Stop and Delete**

- **Stop's** inline confirmation on a sandbox session:
  - Consequence line: "Stop the agent and close N terminal tabs?" (or today's variants), then "The sandbox keeps
    running, and billing, until you suspend or delete it."
  - Buttons: Cancel, **Stop** (danger outline), **Stop and suspend** (danger).

  Stop and suspend ends the agent and its tabs as Stop does, then suspends the sandbox. When session confirmations are
  skipped, plain Stop stays one click, and Stop and suspend is reachable from the row menu.
- **Delete** on a sandbox session always confirms, even when session confirmations are turned off.
  - Consequence line: "Still running — deleting kills the agent and terminates this session's sandbox:" (or the exited
    variant), then "Its files go with it: the checkout, anything not pushed, and the sandbox's snapshots. This can't be
    undone."
  - Confirm button: "Delete session and sandbox".
  - Bulk delete includes the same consequence.
  - A deletion that Tensorlake refuses leaves the session in place, showing the reason, so a retry finishes it, as for
    hosts.

**Session list**

- The row's locality mark is the cube. The meta line reads "dev-box sandbox · owner/repo" (or "dev-box sandbox:~" for a
  home folder).
- A suspended sandbox session is a stale row (grey title), with a detail-line badge "Suspended" carrying the pause
  glyph. This replaces "Host suspended" for these rows.
- The row actions menu:
  - Header summary: the launch settings, then "dev-box sandbox".
  - After the Rename / Mark read / Pin group, a new group with **Suspend**, "Freeze the sandbox; everything continues on
    Resume", or **Resume**.
  - **Clone**'s description becomes "New session in a new sandbox, keep this one".
  - **Stop**'s description becomes "Ends the agent and its processes; the sandbox keeps running".
  - A new **Stop and suspend**, "Ends the agent, then suspends the sandbox", after Stop, in the danger tone.
  - **Delete**'s description becomes "Removes the session and deletes its sandbox".
- No host rail entry and no host filter chip for sandbox sessions. They show under All.

**Settings → Tensorlake**: the creation switch's help mentions sandbox sessions and the 30-minute idle timeout. Recipe
rows say "new host or session" where `prepared-snapshots.md` said "new host", and its "Ready for new hosts" becomes
"Ready for new hosts and sessions". The launcher disables a recipe without a built snapshot instead of building one at
launch, because a build takes minutes and bills; adding a host still builds a missing snapshot as it does today.

### Acceptance criteria

- Launching a sandbox session from the launcher, a template, quick launch, Clone or a recent setup:
  - creates exactly one named sandbox with the reserved prefix, from the recipe's prepared snapshot (ensuring it is
    fresh);
  - creates a hidden session-owned host row, recorded before the sandbox is created;
  - connects, delivers credentials and tunnels, and then creates the session there, with the launch progress showing
    each step.

  A failure at any step leaves nothing billing and nothing listed. Cover it with helm unit tests on the fake Tensorlake
  model at every step. A helm stopped in the middle of a launch leaves a row and sandbox that "Sandboxes not on this
  helm" lists, as described above; cover that too.
- Session-owned hosts never appear in the hosts panel, the host rail, host filters, the launcher's host list, the
  template editor's host list, or the agent-facing `hosts` listing.
- Suspend, Resume, Stop and suspend, and Delete behave as described, from the toolbar, the row menu and bulk actions.
  Farhelm never suspends or resumes a sandbox session on its own. Delete always confirms, whatever the confirmation
  setting. After Delete nothing remains billing: no sandbox, no snapshot, no row.
- A sandbox session survives a helm restart (connects again, or shows Suspended).
- Every agent request from a sandbox session is refused with the stated message, and posts at most one bell notification
  per launch. No approval card appears. Tests cover spawn, create, clone, a lifecycle verb, a template verb and a host
  verb. An agent's Delete or Clone aimed at a sandbox session from another host is refused; other verbs aimed at one
  keep the approval flow.
- "Sandboxes not on this helm" lists a `farhelm-sess-` sandbox that no row on this helm owns, including another helm's.
- The 30-minute idle timeout is what Farhelm passes at creation.
- SPEC.md and SPEC_impl.md are amended first, in the stack's first PR or a leading docs PR, for:
  - the non-goal "Multi-user collaboration and cloud-hosted execution beyond Tensorlake sandboxes added as ordinary
    hosts";
  - "Sessions depend on exactly one thing staying up: their host";
  - "The helm sees exactly the hosts in its registry" (hidden rows);
  - the Tensorlake host paragraphs (sessions as a second owner);
  - the reserved prefixes;
  - the agent-operation contract in "Agent-spawned sessions" and "Local authority and trust between hosts" (refused from
    sandbox sessions);
  - the new notification kind;
  - the launcher, session view, session list and template paragraphs.
- Browser specs on the e2e fake cover:
  - the launcher's recipe group and disabled reasons;
  - the confirmation;
  - the launch-progress step wording, if the fake can drive it;
  - the toolbar's Suspend/Resume and disabled actions;
  - the suspended card and row;
  - the row menu items;
  - Stop and suspend, and the always-on Delete confirmation;
  - templates naming a recipe.

  The fake does not provide a dialable sandbox. Where a state needs a connected session host, find the existing seams
  the Tensorlake host specs use, and log what could not be covered in the browser and how it was covered instead.
- Real-sandbox verification (see the decision). Launch a sandbox session with a managed checkout of a public repository,
  then: run a command in it, Suspend (processes survive and continue after Resume), Stop and suspend, Clone, and Delete
  (nothing left billing). Also confirm the idle timeout value on the created sandbox through the API.
- A changelog fragment (`kind: added`) describes sandbox sessions, with these caveats:
  - experimental;
  - billing while running;
  - explicit Suspend;
  - the 30-minute timeout when Farhelm is away;
  - Delete removes the files;
  - agents can't ask Farhelm to act;
  - Claude's first-run setup may show once per session when using a subscription login rather than an endpoint.
- The last PR updates the Phase 2 TODO.md entry. It removes the entry and adds, in the same bucket and subsection, one
  entry per follow-up this plan left out and still wanted: per-repository warm snapshots with incremental fetch, live
  resize for sandbox sessions, and shared scratch volumes and warm forks (as "Possibly" in the old entry). Keep the old
  entry's design-record pointers on the per-repository one.

## Decisions already made

Settled with the maintainer on 2026-11-07 and 2026-11-08, in conversation and on a planning page with mockups:

- **The model above.** It is one sandbox per session, named, created from the recipe's prepared snapshot. Suspend/Resume
  are explicit, the idle timeout is 30 minutes, and Delete removes everything.
- **Plain Suspend is first-class, and Stop and suspend is offered as well.**
- **No agent operations** from sandbox sessions. The agent gets a plain refusal, and the session gets a quiet bell
  notification so the user knows the agent tried. The near-term TODO entry for the trust work is added by the planning
  PR, not by this plan.
- **No placeholder session.** The launch progress names each step, and a failure cleans up and shows the reason.
- **Toolbar:** Suspend is its own button left of Stop, with pause and play glyphs.
- **While suspended:** only Resume, Clone and Delete work.
- **Destinations:** a managed checkout, or Folder fixed to home.
- **No rail entry.** Sandbox sessions show under All.
- **Naming:** "dev-box sandbox", with a cube mark.
- **Helm restart:** connect as usual.
- **Forgotten sessions are accepted for now.** Auto-suspend is a separate near-term TODO entry, added by the planning
  PR.
- **Claude's first-run setup is out of scope.** The smooth path is an endpoint through a recipe tunnel.
- **No mise** anywhere.
- **Three chained plans**, this one last.
- **Code reviewer:** a single fresh-context Opus 5.5 agent at high effort with the general charter below.
- **No-workhorse mode**, the plans default.
- **Real sandboxes are authorized** for verifying this plan. Asked per plan, the maintainer answered "yes, up to 100 its
  fine" (2026-11-08). So:
  - create at most 100 sandboxes over this whole plan;
  - use the smallest size that works;
  - terminate each one and delete every snapshot it made as soon as its check is done;
  - keep a ledger of every sandbox and snapshot you create (name and ID) in the scratch directory, and sweep from it
    before anything else after an interruption;
  - report the count.

  Never use the maintainer's live Farhelm install or its helm to create or reach sandboxes (root `AGENTS.md`, "The live
  install is off-limits"). Run a throwaway test helm with its own temporary state; its own helm tag keeps its sandbox
  names apart from the maintainer's. Follow `.agents/tensorlake.md` for the ledger and sweep habits. Its "only on
  request" rule is satisfied by this decision, for this plan only. Register any SSH key a test needs with the signed-in
  `tl` CLI, and remove it when done. If the machine has no usable Tensorlake credentials, block with that as the
  question rather than skipping the verification.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:**

- SPEC.md: Topology and host registry; the Tensorlake host and recipe paragraphs; Sessions → Creation, Managed checkouts
  and Lifecycle operations; the Session view; the Session list; Status (notifications); Durability and resume;
  Agent-spawned sessions; Security; Non-goals; "Local authority and trust between hosts".
- SPEC_impl.md's Tensorlake and agent-request sections.
- Root `AGENTS.md`, "Code rules". Host-kind decisions go through the named predicates (`HostKind` in
  `crates/farhelm-helm/src/store.rs` and `crates/farhelm-ui/src/lib.rs`), never scattered kind comparisons.
- Root `AGENTS.md`: "The live install is off-limits", "Sharing the machine with other agents", "Tests and validation",
  "Changelog fragments", "TODO.md and FLAKES.md".
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.
- `website/AGENTS.md`, if docs pages describe Tensorlake hosts. Add a short docs section for sandbox sessions only if
  the docs already cover Tensorlake hosts; name the page in the report.

**Planner proposals** (change them if the code says otherwise, and log a DECISION):

- how the session-owned row is marked (for example an owner column or flag on the Tensorlake binding) and hidden;
- how launch is orchestrated. The recommended way is in the helm: run the session's sandbox through the same setup run
  that Add host → Tensorlake sandbox uses (`crates/farhelm-helm/src/provisioning/tensorlake_add.rs`, which already
  reports named steps that clients can show), then create the session on it, chained in the helm. A client that goes
  away mid-launch then cannot strand a host. A client-side chain like the resume-then-launch path in
  `crates/farhelm-ui/src/list/create_form.rs` is the alternative; it needs a reason, logged;
- how a recipe target is represented in launch requests, templates and recent setups;
- the notification's wording, and whether the refusal travels as the existing unauthorized error kind or a new one;
- the Suspend confirmation rule;
- every UI string above, which is the agreed meaning, not frozen text.

## Outline

The code today, from a read-only map made while planning (verify each before relying on it):

- **Launch:** `crates/farhelm-helm/src/sessions.rs`. `CreateReq` has `host: Option<HostId>` and `github_checkout`. The
  flow runs `create_session` → `create_target` → `host_client`. A host that is not connected is refused. The shared path
  is `do_create_session`, and fresh checkouts go through `create_fresh_session_resolved`.
- **Launch progress** is client-side (`crates/farhelm-ui/src/launch_progress.rs`). The resume-then-launch precedent is
  in `crates/farhelm-ui/src/list/create_form.rs`.
- **Host rows:**
  - The schema and `HostKind` predicates are in `crates/farhelm-helm/src/store.rs`, with `register_tensorlake_host` and
    the Tensorlake columns. About 30 tables cascade from `hosts`, so never rebuild that table.
  - `hosts::host_views` lists every host.
  - The UI host picker is `crates/farhelm-ui/src/list/host_picker.rs` and `list/shared.rs` (`host_options`).
- **Tensorlake lifecycle:** `crates/farhelm-helm/src/tensorlake/lifecycle.rs` (suspend/resume) and `tensorlake/hosts.rs`
  (`delete_sandbox`, `SandboxStatusSource`). Host states are `HostState::Suspended` and `TensorlakeChanging` in
  `crates/farhelm-helm/src/manager.rs`.
- **Agent requests:** `crates/farhelm-helm/src/agent_requests.rs` (`AgentOrigin`, `handle`), and the supervisor relay in
  `crates/farhelm-supervisor/src/service/agent_relay.rs`. Session notifications live in the supervisor's notifications
  service.
- **UI:**
  - session toolbar: `crates/farhelm-ui/src/session_view.rs`;
  - outage card: `SessionUnavailableCard`, with wording from `crates/farhelm-ui/src/hosts.rs`, `stale_session_notice`;
  - list rows and menu: `crates/farhelm-ui/src/list/row.rs` and `crates/farhelm-ui/src/menu_panel.rs`;
  - templates: `crates/farhelm-ui/src/list/templates.rs`;
  - icons: `crates/farhelm-ui/src/icons.rs`.

A likely stack (reshape freely, keeping each PR reviewable and main never broken):

1. `docs:` the spec amendments above.
2. `feat:` session-owned Tensorlake hosts in the helm. This PR carries:
   - the row marker and hiding;
   - the reserved prefix and the orphan list;
   - creation from a prepared snapshot with the 30-minute timeout;
   - Delete's ordering and proof;
   - unit tests.
3. `feat:` launching on a recipe. This PR carries:
   - the request shape;
   - helm orchestration and step progress;
   - failure cleanup;
   - Clone and recent setups;
   - templates' recipe target;
   - the launcher UI;
   - the confirmation.
4. `feat:` suspend and resume from the session: the toolbar, row menu, outage card, list row and status line wording,
   plus Stop and suspend.
5. `feat:` the refusal of agent operations, and the notification.
6. Browser specs, if not folded into the PRs above. Then the real-sandbox verification, the changelog fragment and the
   TODO.md update.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect these:

- `cargo fmt`, plus clippy on the touched crates;
- the helm crate's sessions, agent-request, Tensorlake and provisioning tests, and the UI crate's tests, through
  `python3 scripts/record-test-run.py`;
- the touched browser specs on both engines;
- `dprint check` on changed Markdown;
- the changelog check;
- `python3 scripts/check-agents-md.py` if you add an instruction file or move a heading it watches;
- `python -B scripts/check-test-sleeps.py` when tests change;
- the real-sandbox verification.

## Fallbacks

These departures need the scope reassessment:

- a placeholder session row or other helm-side pending-session state;
- a timer of Farhelm's own for idle suspend;
- any automatic suspend or resume;
- a rail entry or host filter for sandboxes;
- copying a sandbox's disk on Clone;
- per-repository snapshots;
- mise;
- letting any agent operation through.

Block, rather than work around, in any of these cases:

- the helm cannot hide session-owned rows without a `hosts` table rebuild;
- the idle timeout does not behave as described on a real sandbox (it suspends a connected sandbox, or never suspends
  one the helm left);
- verification would need more sandboxes than the decision allows.

Log these as DECISIONs:

- the row marker;
- orchestration (helm or client);
- the request, template and recent-setup representation of a recipe target;
- the notification's dedupe;
- the Suspend confirmation rule;
- what the browser specs could not cover;
- every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-sandbox-sessions-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type,
`$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when a watched filesystem drops
under 10% or under 5 GB free or available memory drops under 10% (whichever comes first), when the number keeps falling,
when the condition recovers, and when the monitor itself fails. In Claude Code a notification is a stdout line of a
monitor started with the Monitor tool; elsewhere, the harness's equivalent. A file update alone is not a notification.

Before relying on the watchdog, verify delivery with a harmless synthetic alert, and verify failure detection by killing
a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a throwaway monitor
without killing it and confirm you are notified within two sample intervals. If any of these notifications is
unavailable or unverified, say so in the log and check the status file at least once a minute. On every path, check
heartbeat freshness before each new build, test run, sub-agent or review launch; a dead monitor, or a heartbeat stale
for two sample intervals, pauses new launches until monitoring is restored.

An alert is an instruction to act: stop launching work, remove jj workspaces, build output and scratch you own that are
no longer needed, wait for or stop the job most likely responsible, and resume only when the watchdog reports headroom.
Record the watchdog's handle, watched paths, status path and delivery mechanism in the log, include its state in every
handoff note, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/sandbox-sessions/<nn>-<short-name>`.
- Avoid churn; within this run, a PR that needs correcting is restructured rather than corrected on top, including PRs
  an earlier run built.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`. PRs stay drafts: never mark one ready and never merge; landing is the monitor's job.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 at high effort, launched
natively or through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log the
DECISION.

### Scope reassessment

Before implementing a substantial departure from the outline (examples under Fallbacks; not a blacklist), or when the
same component has needed repeated corrective review rounds, run a fresh-context review at your own model and effort.
Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered, including every review finding you decided not to follow.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Wording in this file that turns
out inaccurate (a line number, a file name, a class name) is corrected to the truth. A material scope expansion, a
weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or
a log entry is not authorization. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should
know and possible follow-ups go into the report's "things you should know" and "open questions and possible follow-ups"
sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, the real-sandbox verification is recorded in the report, and the last PR replaces the Phase
2 TODO.md entry with its follow-up entries. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12). Never
edit `plans/` yourself.

## Blocked

Blocked on 2026-11-08 (claim 8ec594).

### Sandbox-session launch cannot check SSH key registration in advance

This plan lets a user launch a session into a new Tensorlake sandbox from a recipe, then explicitly suspend, resume or
delete that sandbox. The agreed launcher disables a recipe when Farhelm's SSH key has not been registered with the
Tensorlake account, so a user cannot start a billed sandbox that Farhelm cannot reach.

The existing product specification says Farhelm cannot check that registration: its saved Tensorlake API key cannot list
the account's SSH keys. Farhelm can establish that its own key exists locally, but that does not establish account
registration. Existing Tensorlake host setup learns about a missing registration only when SSH admission fails, after
creating the sandbox. A remembered registration checkbox would not establish the current state either, because the user
can remove the key or switch accounts outside Farhelm.

The implementation has not started. The independent design review found this conflict, and it needs a decision because
silently allowing an unverified registration would weaken the agreed preflight gate.

Options:

- Allow recipe selection when the API key, local Farhelm SSH key and built snapshot exist. Show the existing
  registration command and explanation. If SSH then refuses the key, refuse the launch with the registration remedy and
  delete its newly created sandbox and snapshots; retain an explicit cleanup entry if Tensorlake refuses deletion. This
  can briefly create a billed sandbox before discovering the missing registration.
- Keep the strict prelaunch registration guarantee and defer this plan until Tensorlake provides a way for Farhelm to
  check it with its supported credentials. Adding a different account-login or registration-tracking system would need a
  separate design decision.

Recommendation: take the first option and explicitly amend the launcher requirement. It uses the existing authentication
model and preserves the agreed failure-cleanup behavior without pretending registration was checked.
