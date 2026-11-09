# Approval cards keep their answer controls in view

Written against main at 8e876c14 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where the acceptance criteria below say to change them.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Make the approval cards fit their answer controls, and audit every one of them.** A session-start request grew
> taller than the space the cards get: the requester's session ID filled a full-width row, the managed checkout's
> repository name wrapped one character per line in a narrow column, and the YOLO permission description ran five lines,
> so the Allow/Deny buttons sat below the fold and reaching them meant scrolling the whole card region. The region is
> capped at 55% of the main pane and scrolls as a unit (`crates/farhelm-ui/assets/approval-layout.js`), so any card
> taller than that scrolls its own controls out of reach. Give the request cards a consistent size and position, keep
> the answer controls always visible, and make them look better than the current plain column of labelled rows. Audit
> every request kind, not just the launch card: launch, prompt, rename, stop, delete, restart, open URL, host
> add/edit/remove, and template write/delete. Where content genuinely does not fit, only the relevant long text scrolls
> (the command, the first prompt, a session ID, a long path), never the controls or the dialog as a whole. Seen on a
> session-start request on 2026-11-08.

What is true today, from a read of main at 8e876c14 (line numbers approximate):

- Everything is in `crates/farhelm-ui/src/approvals.rs`. `ApprovalCards` (around 94) is the region; `ApprovalCard`
  (around 468-619) is one shared shell for every kind: a header (badge "Agent request", the heading from
  `action_summary`, "1 of N waiting"), a requester `dl` (from host, asking session title, asking session ID as a
  full-width row with Copy), the kind's rows, kind-specific notes, an error line, and the answer buttons. Per-kind
  content is data: `action_rows` (around 886-1187) builds `CardRow` values (enum around 777: Value, Block, Note,
  Identity, CopyIdentity, Permission) through `launch_rows` (around 1206), `template_rows` (around 1311) and
  `session_rows` (around 1439); `CardDetail` (around 648) renders a row, and `full_width_row` (around 715) decides which
  rows take a whole line. Only the first prompt's block is height-capped today (`.approval-card-prompt`, 20rem).
- The kinds (`ApprovalAction`, `crates/farhelm-proto/src/approvals.rs` around 117; strings from `action_kind` around
  795): `open_url`, `launch`, `launch_with_prompt`, `launch_with_agent_wrapper`, `rename`, `stop`, `prompt`, `delete`,
  `restart`, `template_write`, `template_write_with_shortcuts`, `template_delete`, `host_add`, `host_edit`,
  `host_remove`. The launch verbs (spawn, spawn_inherited, create, clone) and the link flags (local network,
  preapproved, tunnel available) change rows, notes and buttons.
- The answer buttons (around 571-616): Tunnel and open (desktop links with a tunnel), Allow (or "Open add dialog",
  "Open"), "Always allow on <host> only", "Always allow from <host>", Deny (or "Dismiss"). They are disabled for 700 ms
  after the list or selection changes (`ARM_DELAY_MS`).
- Geometry: `crates/farhelm-ui/assets/approval-layout.js` (around 28-44) places the region below the terminal's top (or
  the session top bar), `min(660, pane − 24)` wide and centered, with
  `maxHeight = min(0.55 × pane height, pane bottom − top − 12)`, falling back to the whole viewport under 44px.
  `.approval-cards` (`app.css` around 8896) scrolls as a unit. Only one card is expanded; the other requests are
  `button.approval-waiting` headers below it.
- CSS: `app.css` around 8892-9070. The fact grid is `repeat(auto-fit, minmax(min(100%,190px),1fr))`, which is what
  squeezes long values into narrow columns. There is no `@media` rule for cards; narrow widths rely on the grid and the
  width clamp.
- SPEC.md, "Agent-spawned sessions" (from around 3408), around 3618-3640: one expanded card at the top of the main area,
  oldest first, a waiting count, "Its compact fact grid reflows in a narrow main area, and the whole request region
  scrolls once it reaches its height cap … command text stays full and wrapped", the full-viewport fallback, the 700 ms
  pause, what each card must show, and "session IDs in full-width identity rows with Copy". Around 3642 the own-host
  allow, 3819 host cards, 3759 the create card. SPEC_impl.md around 8724-8742 describes the card layout (regrouped rows,
  the 55% cap, the 44px fallback); around 1065 and 796 the arming and modal exemption.
- Both prompt boxes are capped today (`.approval-card-prompt` applies to "initial prompt" and the prompt card's
  "prompt"), and they are keyboard-scrollable through `PeerBlock`'s `scroll_label` (`crates/farhelm-ui/src/peer.rs`
  around 412).
- Permissions today: launch cards show up to four rows (the mode's heading line from `permission_help`,
  `permission flags`, `workspace trust`, and Farhelm's `YOLO` verdict from `yolo_words` or the own-setting text);
  command launches show `asserted YOLO` and `declared agent` and deliberately no Farhelm verdict (around 968); restart
  cards show mode, flags, trust and asserted YOLO through `launch_rows(Runs::Resume)`; prompt cards already have one
  `CardRow::Permission(glyph)` row; template cards show `permission_label` and, for command templates, `asserted YOLO`.
  `list::session_permission` already classifies a launch into `PermissionGlyph` (Yolo, Guarded, OwnSetting, Unknown,
  Approve, SmartApprove, Chat, AssertedYolo, AssertedNotYolo), with wording in `permission_description` and the mark in
  `PermissionIcon`.
- Session IDs on cards are entry-prefixed references, `<entry-uuid>:<bare>` (`SessionRef`'s `Display`), shown through
  `display_identity`, which spells spaces, formatting characters and escapes distinctly; unit tests pin both the
  distinct spellings and the per-role Copy accessible names ("Copy from session ID", "Copy session ID", "Copy copied
  session ID"). `session_rows`' doc explains that title, ID and host are separately labelled because a title is agent
  text that could imitate the others.
- Tests: Rust unit tests in `approvals.rs` (from around 1468); `e2e/tests/approval-layout.spec.ts`, whose
  `stageApprovals` (around 60) fakes `/api/approvals` and the answer route from JSON fixtures, so any kind can be staged
  (geometry checks around 400, the 48px short viewport around 615, link cards around 702-790, prompt and host cards
  around 791-903); `e2e/tests/helpers/approvals.ts` (`answerCard`) used by real-path specs (`spawn.spec.ts`,
  `github-checkouts.spec.ts`, and others); the docs-shots visual review's approval case
  (`e2e/docs-shots/visual-review.spec.ts` around 3304-3430).

### Acceptance criteria

- Every kind renders in the layout the maintainer chose (layout A, below):
  - a header with the badge, heading and waiting count;
  - one compact "From" line under it: the requesting host in bold, the asking session's title in quotes (ellipsized,
    full text in its hover; today's "title unknown" wording when there is none), the requester's session ID shortened,
    and a "Copy ID" button that copies the full raw ID;
  - the facts as one aligned list, a label column and a value column, one fact per line, never a multi-column grid;
  - paths, repositories, SSH destinations and the container token-file path on one line in the mono font, scrolling
    sideways inside their own row rather than wrapping or pushing the card wider; URLs and permission flags keep
    wrapping in full (their tails are the security-relevant part, and SPEC.md requires a link card to show the entire
    URL);
  - command, initial prompt, prompt, resume, wrapper and template-command text in their own boxes, each capped at a few
    lines and scrolling on its own, each passing `PeerBlock`'s `scroll_label` so it is keyboard-scrollable (single-line
    sideways values get no tab stop; their text stays selectable);
  - an answer bar pinned at the bottom of the card, separated by a rule, with Deny set apart at the right and the other
    buttons in today's order at the left, keeping today's button classes (`approval-allow`, `approval-always-allow`,
    `approval-always-allow-own-host`, `approval-deny`, `approval-tunnel`), which `e2e/tests/helpers/approvals.ts`
    clicks.
- The From line keeps the values apart that separate labelled rows kept apart before: host, title and ID are each their
  own isolated `peer-value` span; Farhelm draws the quotes outside the title's isolate; the ID is mono and the title is
  not; each value keeps an accessible label ("From host", "From session", "From session ID").
- The shortened requester ID shows the start and end of the bare ID (after the entry prefix, which is the same for every
  session on a host), cut on the `display_identity` spelling and never inside an escape token. Its hover and accessible
  name carry the full `display_identity` spelling, which is where SPEC.md's distinct-spelling guarantee now lives. The
  Copy button's visible label is "Copy ID", its accessible name keeps today's per-role wording, and it copies the raw
  reference.
- Every other session ID on a card (the target of a rename, stop, delete, restart or prompt; a copied session) stays in
  full, as a single-line mono value in its fact row that scrolls sideways, with its Copy button; on a stop or delete
  card it is the strongest identifier of what is destroyed.
- Permissions on launch and restart cards are one row, built from `list::session_permission` and
  `permission_description` (plus `permission_help`'s heading for a guarded mode's vendor name), not a second vocabulary.
  Write the glyph → chip → short description mapping into the PR description and the working log, covering every glyph:
  YOLO and Custom (YOLO chip, warning style), own setting (says Farhelm treats it as YOLO), Guarded (vendor mode name),
  Approve, Smart approve, Chat, Unknown (predates classification), and for command launches and their restarts "Asserted
  YOLO" / "Asserted not YOLO", worded as the agent's unchecked claim, never Farhelm's verdict. Workspace trust is a
  second chip line whenever it is stored: "Trusted workspace" with its source ("by Settings in a managed checkout") in
  its description or hover, and "Not trusted" when stored false. `permission flags` stays its own row. A missing YOLO
  chip on a non-YOLO mode is the "not YOLO" answer. Prompt cards keep their single row, restyled; template cards keep
  `permission_label`'s text and `asserted YOLO` as they are (a template's omitted permission means the request default,
  not own setting), restyled with the chip styling only.
- The region's height cap rises from 55% to about 75% of the pane. Within the cap, the card's header and answer bar stay
  in view: when content is taller, the long-text boxes shrink (down to about two lines each, each still scrolling)
  before anything else gives way, and only then does the fact list between header and answer bar scroll. This is pure
  CSS flex layout (header and bar `flex: none`; a body with `min-height: 0` that scrolls; boxes `flex: 0 1 auto` with a
  two-line `min-height`; fact rows not shrinking; boxes outside the facts' grid); the layout script changes only its cap
  constant and fallback threshold.
- The region also holds the late-answer notice above the card and one header per other waiting request below it. The
  waiting headers sit in their own list under the card, capped at about three headers' height and scrolling on their
  own; the late notice is a fixed item; the card gets the rest of the cap. Every waiting request stays reachable.
- Tiny panes: the full-viewport fallback's threshold rises from 44px to the card's pinned minimum (header plus answer
  bar, measured or a conservative constant). Whenever the region is shorter than that minimum, it scrolls as a unit as
  it does today; that is the one exception to the pinned rule, and the existing 48px short-viewport test keeps passing
  under it.
- The card keeps one width and position for every kind (today's width rule and top anchor); its height follows its
  content up to the cap.
- Every kind is audited, including the three the TODO entry does not list (launch with prompt, launch with agent
  wrapper, template write with shortcuts), the launch verbs, and the link variants: each shows every fact it shows today
  (SPEC.md's lists of what each card must show still hold), in the new layout.
- Narrow panes (down to 320px): labels may stack above values; nothing scrolls sideways at the region level; the answer
  buttons wrap but stay visible.
- Peer text keeps its isolation and escaping (`display_peer`, `peer-value`), and every copy button copies the raw ID, as
  today.
- SPEC.md's approval-card passages are rewritten to the new rules: the cap, what scrolls, the fallback exception, the
  shortened requester ID with its hover and Copy ID (replacing the full-width identity row rule for the requester), the
  merged permission row. SPEC_impl.md around 8724-8742 likewise.
- Browser tests in `e2e/tests/approval-layout.spec.ts`, one test per pane width (wide, 650px, 320px) that steps through
  fixtures of every kind with long values (long repository, folder, command, prompt, host name, session title) using the
  staging helper's replace, rather than one test per kind and width: every answer button is inside the visible region
  without scrolling, the region has no sideways scroll, the header stays visible, and only the designated boxes and
  single-line values scroll. Shrink thresholds are read from computed style, not hard-coded: one case where the boxes
  overflow at their cap but fit at their minimum (the body does not scroll), one shorter pane where the body scrolls and
  the answer bar stays inside the region, and one with several waiting requests and a tall card. The Rust `action_rows`
  tests cover "every fact still shown"; the browser tests prove geometry. Existing real-path specs that answer cards
  keep passing.
- `e2e/docs-shots/visual-review.spec.ts` (around 3423-3433) asserts the full requester ID and some container classes in
  the card's DOM. Read `.agents/docs-media.md` and `docs/docs-shots/SPEC.md`, then update those assertions to the new
  layout without recapturing or publishing any image; mention in the report that the published docs shots show the old
  card.
- A `fix` PR (or a short stack) with a changelog fragment. The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, on the planning page, from rendered mockups built on the app's own
stylesheet:

- **Layout A, the aligned list.** The approved mockup of a managed-checkout launch with a first prompt, in a 660px card,
  showed, top to bottom:
  - the header: "Agent request" badge, "Start a new session", "1 of 2 waiting" at the right;
  - the From line: "From **devbox** · “Refactor the provisioning planner” 7f3c9a1e…8f13 [Copy ID]", with a rule under
    it;
  - a fact list with a ~112px label column: On host, Checkout (mono, "GitHub: example-org/infrastructure-tools"), Folder
    (mono), Title (italic "Generated"), Agent ("Claude · opus · high effort" on one line), and Permissions (a YOLO
    warning chip with "Bypasses its approval and sandbox safety checks", then a "Trusted workspace" outline chip with
    "Repository hooks, settings and commands can run without asking");
  - labelled boxes for Command, Initial prompt and Resume in the well style;
  - an answer bar on a slightly darker strip: Allow (primary), "Always allow on devbox only", "Always allow from
    devbox", then Deny alone at the right.

  The alternative, compact "Where"/"Agent" boxes side by side with tabs for prompt, command and resume, was not chosen.
- **Height:** raise the cap to about 75% of the pane; past that, the text boxes shrink and scroll. Keeping 55% and
  scrolling the facts was the alternative.
- **Session ID:** shortened (first and last characters) with Copy ID and the full ID in its hover, rather than in full
  on the From line. This replaces SPEC.md's full-width identity row rule.
- **Permission rows merged** into one row with chips.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold** at the maintainer's request; that changes nothing about how it runs.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md, "Agent-spawned sessions", for what each card must show and the answer
semantics (only its layout rules change); the peer-text rules (`display_peer`, peer values isolated, never Farhelm's own
words); the 700 ms arming pause; modal exemption of the region; the z-index ladder note in `app.css` around 2231; root
`AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`.

**Planner proposals:** reading "a consistent size and position" as one width and position with content-driven height up
to the cap (a fixed tall card would cover most of the terminal for a two-row link card); keeping `CardRow` as the data
model and changing rendering and a few row producers (the permission merge, the requester line) rather than per-kind
markup; the shrink order (text boxes first, then the fact list) in pure CSS; the waiting-header list's own cap; the
fallback threshold; target and copied-source IDs kept in full; URLs and permission flags kept wrapping; template
permission rows only restyled; "about two lines", "about three headers" and "about 75%" are targets to tune, not exact
numbers; Deny set apart at the right.

## Outline

1. `plan/approval-cards-fit/01-card-layout`, typed `fix`: the new shell (From line, aligned facts, single-line scrolling
   values, capped text boxes, pinned answer bar, waiting list), the shrink rules in `app.css` and the cap and fallback
   constants in `approval-layout.js`, SPEC.md and SPEC_impl.md, the layout tests, the docs-shots assertion update, and a
   changelog fragment.
2. `plan/approval-cards-fit/02-permission-row`, typed `fix`: the merged permission row on launch and restart cards and
   the restyled prompt and template rows, its tests and SPEC.md text, the first PR's fragment edited if the change
   shows, and the TODO.md removal.

A single PR is fine if the second is small. Recapturing or publishing the docs-shots approval images is not part of this
plan.

### Validation

`e2e/tests/approval-layout.spec.ts` and the specs that answer cards through `e2e/tests/helpers/approvals.ts`
(`spawn.spec.ts`, `github-checkouts.spec.ts`, `keyboard-scroll`, `modal-trap`, `terminal-replay-rename`, the
provisioning cases that touch cards), `terminal-link-clicks.spec.ts` (it uses the region as a link cover and places
targets beside and below it), and the docs-shots approval case if `.agents/validation.md` says it runs, on the engines
`.agents/validation.md` names, through `scripts/record-test-run.py`; the UI crate's Rust tests (`approvals.rs`);
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.

## Fallbacks

Examples of departures that need the scope reassessment: changing what a card shows (adding or dropping a fact),
changing answer semantics or button set, tabs or collapsible sections, per-kind bespoke layouts, or moving the region.

Decisions to log: the final cap and shrink thresholds, how narrow panes stack, any fact whose placement differs from the
mockup, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-approval-cards-fit-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/approval-cards-fit/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
