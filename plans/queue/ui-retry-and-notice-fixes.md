# Execute three triage outcomes: Replace retries, the uninstall notice, and pastes that leave an item out

Written against main at 4d94b3a3 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `ui-layout-and-escaping-fixes.md`. That plan changes the same two Replace functions this one does
(the header's `replace` closure in `crates/farhelm-ui/src/session_view.rs` and the sidebar's `do_replace` in
`crates/farhelm-ui/src/list/view.rs`, where it adds an explicit listing refresh after a header Replace), and building
both at once would collide in the same success arms. Start from main once that plan has landed, and keep its refresh
behavior intact.

## The goal

Carry out three triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, all `fix code`, in one draft PR. Their headings,
which are also the feedback file names under `review_feedback_queue/`:

- `plain-replace-loses-retry-identity.md`
- `uninstall-forgotten-success.md`
- `recently-modified-image-png-gets-generated-attachment.md`

All three are in the web and desktop UI (`crates/farhelm-ui/`). In product terms:

- If the reply to a Replace is lost (a timeout, a dropped connection) and the user clicks Replace again, the UI sends a
  fresh request key, so a second replacement agent starts in the same folder instead of the first one being recovered.
  This happens from both the session header and the sidebar row menu.
- With two GUI windows open, a window that confirmed an uninstall can announce "Farhelm was removed from <host>" when
  the uninstall actually failed and the other window merely removed the host from the list. Farhelm stays installed on
  the host.
- A paste that carries both a file and an image entry uploads only the file, because an image entry with the clipboard's
  placeholder name (`image.png`, modified in the last 5 seconds) is treated as the clipboard's own rendering of the
  pasted file. A genuine file called `image.png` pasted together with another file is therefore left out silently.
  Nothing on disk is lost, and the code comment claiming every real file is uploaded is wrong.

None of these was reproduced at runtime; all were confirmed or judged by reading the code. Each ledger entry
(Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read
the three entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger
entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every outcome dropped by its gate goes back to the user as Complexity gate says. The recorded Outcome field is never
  rewritten.
- Each outcome's Execution field reads "planned in `plans/queue/ui-retry-and-notice-fixes.md`" when this plan starts
  (the planning PR set it); that is the expected starting state, not a discrepancy. A different Outcome, a different
  plan named, or `complete` means the item changed since planning: leave it alone and say so in the report.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, carries a changelog fragment, and passed the review gate.
- The checks under Validation passed, with recorded browser runs on Chromium and WebKit for the changed behavior.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11):** recorded per outcome in root `TRIAGE_OUTCOMES.md`:

- `plain-replace-loses-retry-identity.md`: the user accepted "fix code with a complexity gate". The ledger's gate: if
  this needs more than retaining the key in the UI's existing operation state, stop and bring it back to the user
  unfixed.
- `uninstall-forgotten-success.md`: the user accepted the small fix (say the outcome is unknown when this window never
  saw the uninstall finish) and added: "fix complexity gated. if the SPEC doesn't say already, we do not generally
  expend any serious complexity to support concurrent GUIs against the same helm." SPEC.md "One GUI at a time" already
  says that ("do not add significant machinery to keep concurrent GUIs consistent"), so no spec change is part of this
  outcome. The ledger's gate: if it needs more than a small change to the notice logic, bring it back unfixed.
- `recently-modified-image-png-gets-generated-attachment.md`: the user chose, instead of the recommended discard, "show
  omitted note with reason". The deduplication itself stays as it is; the misleading code comment is corrected.

**The user's plan-time decisions (2026-10-11):** the user asked to schedule the newly triaged outcomes through the
planning system and, asked how to group them, chose "All new plans" (no additions to the plans already queued). Review
gate: "gpt-6.1-sol high", a single fresh-context agent with the general charter, no swarm. PR shape: "One PR per plan",
so this plan's outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes only.
No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms in the outline below, verified against main at 4d94b3a3, and the dependency on
`ui-layout-and-escaping-fixes.md`. Handling a gate trip by dropping that one outcome from the PR and shipping the rest
follows the precedent of earlier grouped triage plans.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; the rule against tests that modify the test process's own environment variables),
`plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for
test changes, `.agents/narrow-tests.md` for focused runs, `docs/test-run-evidence.md` for the recorder, and
`docs/test-sleep-check.md` for any delay a test adds.

## Outline

Line numbers drift; find the code by name. Verified on main at 4d94b3a3.

- Replace retry identity (`plain-replace-loses-retry-identity.md`): `replace_session` in `crates/farhelm-ui/src/api.rs`
  calls `mint_intent_key()` itself on every call, so a retry is always a new key; its two callers are the header's
  `replace` closure in `crates/farhelm-ui/src/session_view.rs` and the sidebar's `do_replace` in
  `crates/farhelm-ui/src/list/view.rs`. The helm passes the key to an ordinary idempotent create (`do_replace_session`
  in `crates/farhelm-helm/src/sessions.rs`, "Idempotency, and why only the create half needs it"), so a retry under the
  same key replays the first replacement instead of creating another, and a retry after a fully successful replace
  answers 404 because the source is gone. The fix is client-side only: let `replace_session` take the key as an argument
  and have each caller keep it in its existing operation state, bound to the source session id. The create form already
  does this (`intent_key` in `crates/farhelm-ui/src/list/create_form.rs`, "The key otherwise deliberately SURVIVES a
  failure"); follow its rules:
  - keep the key when the outcome is unknown: a transport failure or an unreadable reply (today an `ActionRefusal` built
    from a string rather than read from a response), and a YOLO-confirmation refusal, whose confirmed retry must still
    reconcile with an earlier attempt that may have been accepted;
  - drop it on success and on any other refusal the helm actually answered, so a deliberate new Replace after an
    acknowledged result (including the helm's "both sessions still exist" reply, which names the replacement) gets a new
    key, as the ledger requires;
  - drop it when the retry would no longer be the same intent: a different source, or the user cancelling the prompt and
    choosing Replace with instead. Telling "answered" from "unknown" needs one small field on `ActionRefusal` (or an
    equivalent) set by `ActionRefusal::read`; keep the existing `yolo_confirmation` semantics.

  The sidebar shares one `do_replace` across rows, so its retained key is keyed by source id (a small map, or a single
  slot naming its source); the header keeps one slot for its session. Leave `replace_session_with` (Replace with, which
  goes through the create form's own key handling) alone. Tests: in `e2e/tests/replace.spec.ts`, intercept the first
  `POST /api/sessions/<id>/replace` with `route.abort()` (the request never reaches the helm, so the source stays),
  retry Replace, and assert that the second request carries the same `intent_key` as the first; once from the sidebar
  row menu and once from the session header. That key equality is the real check. "Exactly one replacement session"
  proves nothing on its own here, because the aborted first request never reached the helm, so only one create could
  ever have happened; assert it only as a sanity check alongside the key, never instead of it. A unit test of the
  keep/drop rule is welcome if the rule ends up in a small pure function. Gate (bring back): anything beyond retaining
  the key in those two callers' state and threading it through `replace_session`, for example needing helm changes.
- Uninstall notice (`uninstall-forgotten-success.md`): `crates/farhelm-ui/src/hosts.rs` keeps `uninstall_notices`, one
  `UninstallNotice` (defined in `crates/farhelm-ui/src/provisioning.rs`) per uninstall this window confirmed, registered
  by the row's provisioning panel before the POST and withdrawn on a definite refusal or when the panel reads a failed
  run of that uninstall. An effect in `hosts.rs` shows "Farhelm was removed from <host>; its data remains in <dir> on
  that host." (`uninstall_notice_parts`) whenever such a host leaves the list. A same-window Remove already drops the
  entry (`on_remove_confirm`); a Remove from another window cannot. The ledger asks that, when this window never
  observed the uninstall finish, the notice say the outcome is unknown rather than that Farhelm was removed. Check first
  whether a successful uninstall is reliably observed by the panel before its host leaves the list: a successful run
  forgets the host as its last step (`a_confirmed_uninstall_runs_in_order_and_forgets_the_host` in
  `crates/farhelm-helm/src/provisioning.rs`), and the hosts.rs comment says the helm keeps no result for a forgotten
  host, so the ordinary single-window success may also reach the effect with no observed completion. If this window can
  tell a finished uninstall from a host that merely disappeared using what it already reads (for example a recorded
  "observed the run succeed" flag on the notice, set by the panel, if the panel does see success in practice), make the
  unobserved case show an "outcome unknown" notice that still says where the data would remain and that the user should
  check the host. If it cannot, so that the fix would turn the ordinary successful uninstall into "unknown" as well,
  that is a gate trip: bring it back with the realistic options (a helm-side record of the final uninstall result that
  survives forgetting the host; hedged wording for every uninstall notice; leaving it as accepted multi-window behavior)
  and a recommendation. Add a component or browser test if one fits the existing provisioning fixtures. Gate (bring
  back): anything beyond a small change to the notice logic in those two files.
- Omitted-paste note (`recently-modified-image-png-gets-generated-attachment.md`): in
  `crates/farhelm-ui/assets/terminal.js`, `payloadFrom` puts an entry in `images` when `chooseName` (in
  `crates/farhelm-ui/assets/clipboard-name.js`) returns `null` for a placeholder-named fresh image, and `accept` uploads
  only the winning flavor's bucket, so when a file wins, every image entry is dropped without a word. The policy's
  classification and texts come from `crates/farhelm-ui/src/attachments.rs` (`classify`, the `"text"` map with
  `DIRECTORY_TEXT` and its siblings, and the test that lists every text key). Add a note to the existing visible failure
  list (`fail(fillTemplate(...))` in `accept`, the way a dropped directory is reported) that names the omitted item and
  says why: it looked like the clipboard's own copy of another pasted file. The text lives in `attachments.rs` beside
  its siblings and is added to that key-list test. Do not change which items are uploaded. The ledger and the user
  ("show omitted note with reason") ask for the note whenever the heuristic leaves an item out, so that is the fix:
  always show it. Copying an image file in a file manager puts the file and a rendering of it on the clipboard together
  (the `classify` docs), so the note also appears on that ordinary paste; word it so the ordinary case reads harmlessly,
  for example "left out image.png, which looked like the clipboard's copy of holiday.png" (name the uploaded file it was
  taken for when there is one). A suppression rule that keeps the note off plainly-ordinary pastes (for example, note
  the dropped image entries only when they outnumber the image-typed files among the uploaded ones) is a planner
  proposal you may consider, but it is not a gate condition and must not replace the literal fix. If the note on
  ordinary pastes seems like unacceptable noise, ship the always-on note and record that in the report as an open
  question for the user, with the suppression rule as an option, rather than dropping or narrowing the fix. Correct the
  `classify` doc comment that claims a genuine `image.png` "costs a name rather than a byte" and that distinct real
  files are all uploaded. Tests: extend `crates/farhelm-ui/js-tests/` (or `e2e/tests/terminal-attachments.spec.ts`) with
  a payload holding a non-image file plus a fresh `image.png` image entry, and with the ordinary
  image-file-plus-rendering payload, asserting in both that the note names the omitted item and its reason and that only
  the file is uploaded. Gate (bring back): the change needs more than the paste classification and its texts.
- Changelog and type: all three change what users see, so the PR type is `fix:` and it adds one fragment under
  `releasing/changelog.d/` per root `AGENTS.md` (Releases and the changelog), written for someone running Farhelm per
  `releasing/EDITORIAL_GUIDANCE.md`, validated with `python3 releasing/check-changelog.py format`.

The complexity gate for every outcome is a change confined to the files the bullet names, plus tests, docs and
bookkeeping. A helm or supervisor change, a new mechanism, or a design decision the ledger did not anticipate is a gate
trip.

### Validation

Follow root `AGENTS.md` "Finishing work"; no full battery.

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop` (the UI crate's desktop build compiles nowhere else). That check needs
  the webkit2gtk/gtk development packages (the CI job's apt list); if they are missing on the executing machine, report
  it as skipped with that reason rather than installing anything.
- Focused Rust unit tests for the changed modules (`attachments.rs`'s policy and text tests, any new unit test of the
  Replace key rule), through `scripts/record-test-run.py` with `--runner nextest`, `--kind development`, `--tmux none`,
  and a selection naming exactly those tests.
- `cd crates/farhelm-ui/js-tests && node --test` if `terminal.js` or `clipboard-name.js` changes.
- Browser runs through the recorder on Chromium and WebKit, after `cargo build` and the dx web build root `AGENTS.md`
  names, narrowed with Playwright's `-g` (and the spec file) to the tests you added or changed plus the existing tests
  covering the touched code: the Replace tests in `replace.spec.ts`, the uninstall tests in `provisioning.spec.ts`, and
  the paste tests in `terminal-attachments.spec.ts`. Not whole spec files and not the full suite; widen only for a
  concrete risk the narrow selection leaves uncovered, and record the selection.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md` if any browser
  or Rust test changes; zero unannotated delays. Apply `.agents/test-authoring.md` to every changed test.
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`, the changelog
  fragment).

A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the retained
run, re-run narrowly, and do not log a flake from one observation.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-ui-retry-and-notice-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/ui-retry-and-notice-fixes/01-ui-fixes`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`fix:` here, with
  the changelog fragment described in the outline).
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field (it starts as
  "planned in `plans/queue/ui-retry-and-notice-fixes.md`") to `complete` with the jj change ID, bookmark and PR URL. The
  recorded Outcome field is never rewritten. Record the change ID and bookmark before creating the PR, then add the URL
  to the same change and push again; no separate bookkeeping PR. An outcome sent back to the user is handled as
  Complexity gate says.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome carries the user's complexity gate, stated in its ledger entry and summarized under Requirement sources;
for all three, a trip goes back to the user. When a fix turns out to need significantly more than the outline describes
(a new mechanism, a helm or supervisor change, a change well outside the files the outline names, or a product or design
decision), do not grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback
file and index line, never rewrite its recorded Outcome field, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution
field to `pending`, waiting on the user, followed by what you found and the realistic options with a recommendation (see
`installer-startup-prune.md` in the ledger for the shape). Name it among the open questions in the plan's report.

The rest of the PR ships. Block per Unattended fallback only when every outcome in this plan was dropped back to the
user, since then there is no PR worth delivering. Code that turned out to be already fixed on main is not a gate trip:
set its Execution to "`complete`: already fixed by <commit>" with the change ID, bookmark and PR URL, leave the recorded
Outcome as it is, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. If the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Ask the reviewer to check in particular that: a Replace retry after an unknown outcome reuses its
key while a Replace after an answered result, a different source, or Replace with gets a new one; the YOLO-confirmed
retry still reconciles with an earlier attempt; the ordinary single-window uninstall still announces its success
unchanged; every item the paste heuristic leaves out gets a note naming it and why, worded harmlessly for an ordinary
image-file paste (file plus rendering); and which items are uploaded is unchanged. Where you disagree with a finding,
decide on the merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill
owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a helm-side record of
uninstall results, a new key registry shared across components, a rework of the paste classification, edits outside the
files the outline names beyond tests, docs and bookkeeping), and whenever the same component has needed repeated
corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the ledger
entries, the user's decisions above, this outline, the current diff and the proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how a Replace refusal is classified as answered or unknown, where each caller
keeps its retained key and when it is dropped, whether a successful uninstall is observed before its host leaves the
list (with the evidence) and what the unknown-outcome notice says, the omitted-paste note's wording (and, if you
considered the suppression rule, why it was or was not adopted and whether ordinary-paste noise went to the report as a
question), the Conventional Commit type and changelog wording, every outcome dropped by its complexity gate and why, and
every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is its complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate (and hands every one that did back to the user as its gate says), meets the acceptance criteria above,
and passed the review gate, with every outcome's `TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the
carried-out outcomes removed. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
