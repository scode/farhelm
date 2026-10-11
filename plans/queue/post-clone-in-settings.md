# The all-hosts post-clone command in Settings

Written against main at 95bb60a7 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a SPEC change the user decided on, which this plan makes.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Put the post-clone command in Settings": a managed checkout can run a shell command
right after Farhelm clones the repository (installing dependencies, say), but the only way to set it is
`farhelm helm checkout-config set-post-clone` on the helm's machine. Make the all-hosts post-clone command editable in
the app's Settings dialog, beside the checkout folder setting, keeping the existing meaning of an empty command
(explicitly disabled) versus no command. Per-host overrides stay command-line only.

Acceptance criteria:

- Settings shows the all-hosts post-clone command in a multi-line field with an explicit save, under the checkout folder
  field. The user can set it, change it, or empty it and save to remove it, and sees the helm's message when it refuses
  a value (the 16 KiB cap is the one refusal today).
- An empty field, saved, leaves the all-hosts command unset. Opening Settings never rewrites the stored value: a command
  line `set-post-clone ""` (explicitly disabled) stays as it is unless the user saves.
- Per-host overrides, of the folder or the command, never appear in the UI or in any reply the browser can read.
- A save takes effect for checkouts accepted after it, never for an already accepted one or its retries, which is what
  the existing configuration revision already guarantees.
- SPEC.md and SPEC_impl.md say what now holds, in the PR that changes it.
- Each `feat`, `fix`, `perf`, `style` or `revert` PR has its changelog fragment. The last code PR removes the TODO.md
  entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-10):** plan this TODO entry.

**The user's decisions (2026-10-10):**

- Reverse the spec rule that keeps the post-clone command out of the browser. Today SPEC.md "Managed checkouts" says
  "Browser-readable configuration replies contain only the all-hosts folder, never hook text or host overrides", and the
  code repeats that in several comments. The user was told that with this change anyone signed in to the helm in a
  browser can read and set a shell command that runs on every host after each managed checkout, and that this is
  arguably no new power because the launcher already lets a signed-in browser run any agent command on any host. They
  chose to amend SPEC.md and SPEC_impl.md and proceed. Per-host overrides stay command-line only and stay out of the
  browser. (A write-only variant, where the browser could set the command but never read it back, was offered and not
  chosen.)
- An empty field, saved, means unset: "Empty the box and save, and the all-hosts command is gone. Nothing else to choose
  in the UI." The user was told that for the all-hosts setting "unset" and "explicitly disabled" behave the same (no
  hook runs either way) and differ only for per-host overrides. No switch for choosing between them.
- The field is a monospace text box a few lines tall with an explicit save button, like the checkout folder field. Its
  help says the command runs in each new managed checkout, on that checkout's host, before the agent starts, and affects
  future checkouts only.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Managed checkouts" (the paragraph that says the folder can be set in Settings and that per-host overrides and
  the post-clone command remain command-line only, and the sentence about browser-readable replies quoted above) and the
  Settings dialog paragraph that lists what the dialog holds. SPEC_impl.md's paragraph on the checkout folder route
  ("The browser reads only the global folder, so neither host overrides nor the executable post-clone command enter its
  reply") and its CLI section. The user's decision above overrides the browser exclusion for the all-hosts command only.
- SPEC.md on hooks: "A checkout is not permission to run a hook unless the maintainer configured that hook", and a
  completed preparation restarts without repeating clone or hook. Both stay true.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; the rule against tests that modify the test process's own environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.
- A separate plan, `text-fields-no-spellcheck`, turns off spell checking, autocorrect, autocapitalization and
  autocomplete in every text field. Give the new field those attributes from the start, whichever plan lands first.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at 95bb60a7.

## Outline

Line numbers drift; find the code by name.

Here's how the checkout folder works today, which this plan mirrors. The store
(`crates/farhelm-helm/src/checkout_config.rs`, SQLite tables `checkout_config` and `checkout_config_host`) already holds
the all-hosts post-clone command with three states: NULL (unset), `""` (explicitly disabled), and a command.
`set_checkout_post_clone` enforces the 16 KiB cap (`MAX_POST_CLONE_BYTES`) with an error that never echoes the command;
`clear_checkout_post_clone` writes NULL; both bump the configuration revision, and `watch_revision` publishes any change
to clients within about three seconds. The helm resolves the command at create time and refuses a create whose preview
was made under an older revision, and the supervisor freezes the resolved command into the durable launch spec, so a
save only affects new checkouts. The checkout folder is exposed as `GET`/`PUT /api/checkout-root` (body
`CheckoutRootBody { root: Option<String> }`, `deny_unknown_fields`, required but nullable), registered in
`crates/farhelm-helm/src/lib.rs` behind device-session authentication, and edited by `CheckoutRootField`
(`crates/farhelm-ui/src/checkout_root.rs`) in Settings (`crates/farhelm-ui/src/settings.rs`).

### PR 1, `feat`: the post-clone command in Settings

One PR, unless splitting the helm route from the field reads better; log the DECISION either way.

- **Helm route.** Add `GET`/`PUT /api/checkout-post-clone` beside `/api/checkout-root`, with the same authentication and
  body discipline. A separate route, rather than a field added to `/api/checkout-root`, keeps the folder route's reply
  shape (and its tests asserting that shape) unchanged and keeps the two explicit saves independent. The body, for both
  GET and PUT, is `{ "command": string | null }`, mirroring the folder route: `null` is unset, `""` is explicitly
  disabled, anything else is the command, so the three stored states stay apart without an extra flag. PUT with a string
  (including `""`, which is the command line's existing contract for "disabled") calls
  `set_checkout_post_clone(None, …)`; PUT with `null` calls `clear_checkout_post_clone(None)`. Neither reads nor returns
  a host override. Check the 16 KiB cap in the handler and return 400 with the message, as the folder route does for an
  invalid path: the setter's bare error would otherwise go through the helm's general error mapping and come back as a
  500.
- **Comments that become false.** Several docstrings and comments say the browser never sees the post-clone command: the
  module docs of `checkout_config.rs` and the folder route's own docstrings there, the route registration in `lib.rs`,
  the `CheckoutRootBody` mirror in `crates/farhelm-ui/src/api.rs`, `crates/farhelm-proto/src/lib.rs`,
  `crates/farhelm-helm/src/sessions.rs` (several places), and the module doc and dialog docstring of
  `crates/farhelm-ui/src/settings.rs`. Grep for the rest. Each one should say what now holds: the browser reads the
  all-hosts command through its own route, and still never a host override. Where a comment is about a specific reply
  (the checkout preview, a session record) that still carries no hook, keep that part.
- **Settings field.** Under the checkout folder field, a monospace multi-line field with a save button and an inline
  outcome line, following `CheckoutRootField`'s behavior: a draft that survives a refused save, a lock while loading or
  saving, a save that completes even if Settings is closed mid-save, a refetch after success, `role=alert` for a refusal
  and `role=status` for success. Unlike the folder field, Enter inserts a newline (the folder field suppresses Enter so
  it cannot submit the launcher form; this field is never in the launcher), and the value is not trimmed beyond treating
  an all-whitespace field as empty. Saving an empty field sends null. An explicitly disabled value shows as an empty
  field with no extra note (the user decided there is nothing to choose between the two in the UI), and nothing is
  written unless the user saves. Share code with `CheckoutRootField` where that falls out naturally; do not force a
  generic component if the differences make it awkward. Log the DECISION.
- **Help text.** The command runs in each new managed checkout, on that checkout's host, after cloning and before the
  agent starts; changes affect future checkouts only; a host given its own command from the command line keeps it.
- **Hover text.** SPEC.md requires hover text on every clickable control, and `e2e/tests/tooltip-coverage.spec.ts`
  checks the Settings dialog. Give the new save button a tooltip. The checkout folder field's save button (and any retry
  button) beside it may lack one too, which would make that spec fail on main already; if so, fix those in the same PR,
  since they sit in the same dialog.
- **SPEC.** Amend SPEC.md "Managed checkouts" so the all-hosts post-clone command, like the folder, can be set in
  Settings or with the command line, and browser-readable configuration replies may contain the all-hosts folder and
  post-clone command but never a host override. Add the field to SPEC.md's Settings dialog paragraph. Update
  SPEC_impl.md's checkout-folder route paragraph to cover the new route and the changed exclusion, including its "no
  host or hook fields" wording.
- **User docs.** `docs/github-checkouts.md` describes the hook as command-line only; update it. On the docs website,
  `website/src/content/docs/docs/using/start-a-session.mdx` describes the command (without the word "post-clone", so a
  grep for it misses the page); update it, after reading `website/AGENTS.md` and `website/EDITORIAL_RULES.md`. Both
  places may also still describe the checkout folder as command-line only; correct that while there.
- **Tests.**
  - Helm route tests beside `mod root_route_tests` in `checkout_config.rs`: set, change and clear; a refused value (over
    the cap) leaves the stored value and revision unchanged and returns the message; PUT `""` stores explicitly disabled
    and reads back as `""`; GET carries no host override (seed host overrides with marker strings and assert they are
    absent, as `get_exposes_only_global_root` does); 401 without a session and 422 for a missing or extra field. Keep
    the folder route's existing tests passing unchanged.
  - A browser test in `e2e/tests/settings.spec.ts` mirroring "settings saves and clears the all-hosts checkout folder"
    : save, refusal, reopen, and empty save clears. Restore the previous value in `finally`. The Playwright
      configuration runs one worker against one shared helm, so this cannot race `e2e/tests/github-checkouts.spec.ts`,
      which sets the hook from the command line. Mirror "a pending checkout folder save survives closing settings" only
      if the new field does not share that code path with the folder field.

The PR removes the TODO.md entry this plan covers. Changelog: `kind: added`, saying the command can now be set in
Settings and that per-host overrides are still command-line only.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm --bins -- -D warnings` since helm code changes;
- the helm's checkout-config tests through `scripts/record-test-run.py` with nextest (`--tmux none` unless the selection
  uses tmux);
- `e2e/tests/settings.spec.ts` and `e2e/tests/tooltip-coverage.spec.ts` on Chromium and WebKit through the recorder,
  after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, and the website build if a `website/` page changed;
- `python3 releasing/check-changelog.py format`.

Take a Playwright screenshot of Settings with the new field (empty, with a command, and refused) and look at it. Do not
run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-post-clone-in-settings-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/post-clone-in-settings/<nn>-<short-name>`, numbered in the order under Outline (for example
  `plan/post-clone-in-settings/01-post-clone-setting`).
- One PR per item under Outline, in that order, as one linear stack. Keep them bite-sized and avoid churn: code added in
  one PR and deleted in a later one means the stack should have been shaped differently. Within this run, a PR that
  needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the PR's part of the outline.

For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Ask the reviewer to confirm that no reply the browser can read carries a per-host override of the
folder or the command, that a refused save leaves the stored value and the revision unchanged, and that the three stored
states (unset, explicitly disabled, a command) survive a round trip through the UI without the user changing them.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a protocol change, per-host settings
  in the UI, a change to how the command is frozen or run, a generic settings-field framework, or edits well outside the
  files the outline names beyond tests and bookkeeping (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- whether the folder field's existing buttons needed hover text;
- whether the field shares code with the checkout folder field;
- whether the route and the field are one PR or two;
- each Conventional Commit type and changelog kind;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: exposing a per-host override, a write-only
command the browser cannot read back, or a UI switch between unset and disabled.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
