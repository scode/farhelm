# Remember the feedback contact: Send feedback offers to reuse the last contact

Written against main at 07aa85e5 on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

## The goal

Send feedback (SPEC.md, "Feedback") opens a dialog with a required message and an optional field for how to reach the
user. That field starts empty every time, so someone who sends feedback more than once retypes their contact. After this
plan, the dialog offers to remember it, and the helm remembers it in its shared preferences so the desktop app and the
web UI both see it.

The TODO.md entry this plans, verbatim:

> **Remember the feedback contact.** Send feedback's optional field for how to reach the user starts empty every time,
> so someone who sends feedback more than once retypes it. When a contact is entered, show a "Re-use for future
> feedback" checkbox, on by default; when it is checked, a successful send remembers the contact and the next feedback
> dialog starts with it filled in. Decide where it is remembered (the helm, so the desktop app and the web UI share it,
> or the browser) and say so in SPEC.md's Feedback section.

Acceptance criteria:

- While the contact field is not blank, the dialog shows a "Re-use for future feedback" checkbox, checked by default. It
  has hover text, like every other dialog control (the tooltip-coverage browser spec checks this).
- When the dialog opens and a contact is remembered, the field starts with it and the checkbox shows checked.
- After a successful send, the remembered contact becomes exactly what was sent when the box is checked and the field is
  not blank, and is cleared otherwise, including when the user emptied a prefilled field (Decision 2). A failed send
  changes nothing. The remembered value is written after every successful send, even when it looks unchanged (outline,
  "UI").
- The contact is remembered in the helm's shared preferences row (Decision 1), read by every client once after it
  authenticates, like the other shared preferences. A dialog shows the copy its client read; another client's later
  change shows up after a reload. SPEC.md says this.
- The feedback request itself is unchanged: nothing new goes to the feedback endpoint, and the helm's feedback route
  still stores nothing. The contact reaches the helm only through the preferences write after a successful send.
- No log line, error message or refusal carries the contact, matching SPEC_impl.md "Feedback forwarding".
- SPEC.md's Feedback section says where the contact is remembered and how the checkbox behaves; SPEC.md's Errors and
  diagnostics list of best-effort preference fields includes it; SPEC_impl.md's preferences-row note lists the column,
  and "Feedback forwarding" gains a sentence (not an exception) saying the dialog remembers the contact through the
  preferences row. The docs page `website/src/content/docs/docs/using/send-feedback.md` mentions it, following
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md`.
- A changelog fragment under `releasing/changelog.d/` (kind `added`) in the same commit as the change.
- The PR removes the TODO.md entry "Remember the feedback contact".

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Where:** the helm's shared preferences row (`helm.db`), so the desktop app and the web UI share it. The maintainer
   accepted that the helm then stores the contact and that every authenticated client can read it through
   `GET /api/preferences`.
2. **When it is forgotten:** after a successful send, remember what was sent if the box is checked, otherwise clear it,
   including when the field was emptied. A failed send changes nothing.
3. **No downgrade path, accepted.** Moving `helm.db` to the next schema version means an older helm refuses to open the
   database. The maintainer was told and accepted it (SPEC.md, "Upgrade compatibility and client scale", requires
   surfacing this). The upgrade path must stay intact: an additive column on the forward migration ladder.
4. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
5. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision, grounded in main at 07aa85e5 and checked by a fresh-context planning
review. Line numbers are approximate.

**Helm.** Add a nullable `feedback_contact` column to the `preferences` row in `crates/farhelm-helm/src/store.rs`
(`SCHEMA_VERSION` 42 today; if another plan has bumped it by the time you run, take the next free version and renumber).
Follow the precedent of the host-confirmation columns (schema 32/33): the migration ladder step, the `Preferences` and
`PreferencePatch` fields (`deny_unknown_fields`, the double-option clear), and the downgrade fixtures that drop
preference columns. `GET`/`PUT /api/preferences` (`crates/farhelm-helm/src/preferences.rs`) carry it; `null` clears.
Validate only the length, with the proto cap (`FEEDBACK_CONTACT_MAX_CHARS`, 200 code points, in
`crates/farhelm-proto/src/feedback.rs`), modelled on the `last_selected` refusal, which states the cap and does not echo
the value. Do not copy the `list_sort` refusal, which echoes its value: the UI logs the helm's refusal text and those
logs are forwarded to the helm. Do not add a blank refusal; the UI never writes a blank (a blank field clears instead).

**UI write path.** `crates/farhelm-ui/src/api.rs`: `Preferences` gains the field (it has no `deny_unknown_fields`, so an
old page against a new helm ignores it). The preference write queue has no way to send a clear today: every
`PreferenceValue` variant carries a concrete value. Add `FeedbackContact(Option<String>)`, whose wire value is `null`
for `None` and whose overlay sets the seeded field to `None`, and wire it into every place a field is listed: the
`PreferenceField` enum and its key, the `PreferenceWrites` slot and its `field`/`dirty` matches, and the field list in
`seed_with_local_changes`. Missing that last list silently drops a failed contact write instead of replaying it after
reauthentication.

**UI dialog.** `crates/farhelm-ui/src/feedback.rs` reads `list::SharedPreferences` (provided by `PreferencesGate` in
`crates/farhelm-ui/src/lib.rs`; `settings.rs` is the precedent) to seed `contact` and a `remember` signal. The checkbox
shows while the contact is not blank. On `Ok` from `send_feedback`, compute the D2 value with a small pure function, set
the shared signal, and call `store_preference` every time: the client's copy was read at authentication and may be
stale, and the write queue's docs (`FieldWrite::recorded_since_send`) say a fresh choice needs its own write even when
it matches what is already there. The write is best effort: failures are logged (without the value), not shown.

**Docs.** As in the acceptance criteria. Keep SPEC_impl.md's "the helm adds nothing, trims nothing, and stores nothing"
for the feedback route and add the sentence about the preferences write after it, so nobody concludes the route writes
to `helm.db`.

**Tests (proposal).** Helm: store and migration tests next to the existing preference-column tests, and route tests for
the cap (including that the refusal does not contain the value) and for clearing with `null`. UI: a unit test of the D2
rule. Playwright, `e2e/tests/feedback.spec.ts`: the suite shares one helm, and the existing success test sends a contact
and succeeds, so with the box on by default it would leak a remembered contact into every later test. Call
`resetPreferences` in `beforeEach`, add the field to `resetPreferences` and to `patchPreferences`' patch type in
`e2e/tests/helpers/fleet.ts`. Add three cases: a send with a contact, then reopen, shows it prefilled and checked;
unchecking, sending, and reopening shows it empty (the first time any client sends a preference clear); a failed send
leaves the remembered value unchanged. Run the changed Playwright specs on Chromium and WebKit through the recorder per
root `AGENTS.md`.

**Size and stack.** Small. One PR (`feat`), carrying the helm field, the UI, the docs and the fragment, so spec and code
agree at every commit.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-remember-feedback-contact-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/remember-feedback-contact/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The change is a `feat` and carries a changelog fragment under `releasing/changelog.d/` (kind
  `added`) in the same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust or browser tests change.
- The last code PR removes the TODO.md entry "Remember the feedback contact".

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, a nextest selection
of the helm's store, migration and preferences tests and the `farhelm-ui` crate's `api` and `feedback` tests,
`cargo check -p farhelm-ui --features desktop` (the dialog is shared with the desktop build), the changed Playwright
specs (`feedback.spec.ts`, and `tooltip-coverage.spec.ts` for the new checkbox) on Chromium and WebKit through the
recorder, the website build for the docs page, and `dprint check` on changed files. Say in the report which checks ran
and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands: a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at high
effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm. Both
get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (browser storage, a helm-side write from the feedback
route, a second preferences endpoint; these are examples, not a blacklist), and whenever the same component has needed
repeated corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the
request, the decisions above, this outline, the current diff and the proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the schema version taken, the refusal wording, how the dialog seeds and writes
the value, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: remembering the contact anywhere but the helm's preferences row (browser storage included); changing what the
feedback request carries or making the feedback route write to `helm.db`; a migration that is not additive on the
forward ladder; and any log line or error text that would carry the contact.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
