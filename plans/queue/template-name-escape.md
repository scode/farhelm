# Escape an older template's session name in the template editor

Written against main at 0e7c3dd5 on 2026-10-16. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The Templates editor shows a template's session name raw in an editable input, even when the name holds invisible or
direction-changing characters. Since #2087 the editor shows such characters in a template's command, resume command,
model and folder as visible escapes (`<U+200B>` and so on) inside a read-only input, with a hint that the value is
replaced by removing the field with ✕ and adding it again. Give the session name the same treatment.

The TODO.md entry (Near term, "Escape the session name in \"save as template\"") says the raw name arrives from a cloned
remote session through "save as template". Planning found that route cannot deliver one: saving a template refuses a
session name with these characters (the panel's own check and the helm's both apply `check_template_shape`), so such a
name can only reach the editor from a template stored before that rule came in on 2026-10-11 (#1932; the 2026-10-10
rule, #1811, covered only the launch and resume commands). The maintainer accepted that correction; the fix is the same
either way.

### Acceptance criteria

- **Same treatment as #2087.** When a template's session name contains a character that
  `farhelm_proto::text::is_presentation_unsafe` flags, the editor's session-name input shows the `display_peer`
  spelling, is read-only, and shows the same "contains characters this editor cannot show directly ... Remove this field
  with ✕ and add it again" hint. The stored bytes are never replaced by the escaped spelling: typing into the read-only
  input changes nothing, and saving another field keeps the raw name (which the existing save refusal then refuses, as
  it already does today for a raw unsafe name).
- **Removing and re-adding** the session-name field restores ordinary editing, exactly as for the other escaped fields.
- **Safe names are unaffected**: an ordinary name stays an ordinary editable input.
- **Tests:**
  - extend the Rust unit test `unsafe_editor_values_require_read_only_escaped_display`
    (`crates/farhelm-ui/src/list/templates.rs`) to cover `Field::Name`;
  - in `e2e/tests/templates.spec.ts`, add `"name"` to the loop of "older template ${field} stays read-only without
    bypassing save refusal" (an older template supplied at the list API boundary, since the current API refuses to store
    one): its escaped name is read-only, cannot be edited in place, and its save is refused without a write. Run it on
    Chromium and WebKit.
  - The existing "older template name deletion warns before permanently deleting" test fills "repaired draft" into the
    unsafe name's input. Once that input is read-only, Playwright's `fill` waits for an editable element and the test
    fails. Adjust it so it still checks what its comment says (a repaired draft cannot make the stored snapshot
    restorable) by removing the name field with ✕, adding it again and typing; that makes it the one browser test of
    remove-and-re-add for the name. Do not weaken what it checks.
- **Docstrings:** the docstrings that list the escaped fields by name (on `escaped_read_only_value`, on `FieldEditor`,
  and on the unit test) name the session name too.
- **No spec or docs change** is expected: SPEC.md's rule that host-supplied text is shown escaped outside a terminal
  already covers session names, and neither SPEC.md, SPEC_impl.md nor the docs website describes the editor's escaped
  fields (#2087 changed none of them). If you find a passage that does list them, update it.
- **Changelog fragment**, `kind: fixed`, with an entry that says what changed for someone running Farhelm.
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-16):** fix it the #2087 way (escaped, read-only, replaced via ✕) rather than an editable
Rename-style escape; the TODO entry's route was wrong and the plan says so; review gate: one fresh-context gpt-6.1-sol
reviewer at high effort, no swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md's rule that host-supplied text is shown escaped everywhere outside a
terminal; SPEC.md's template save rule (session names and commands must not contain these characters; older templates
still launch); `.agents/test-authoring.md` for the tests.

**Planner proposals:** add `Self::Name` to `Field::escaped_read_only_value` and wire the name input's `value` and
`readonly` the way the destination input already is; reuse the existing hint.

## What planning found

- `Field::escaped_read_only_value` (`crates/farhelm-ui/src/list/templates.rs`, near its doc comment "Unsafe text is a
  display-only escape, never a replacement draft value") covers Command, Resume, Model and Destination, not Name.
- The `Field::Name` arm of the field editor renders `input.templates-session-name` with
  `value: current.name.unwrap_or_default()` and an `oninput` that writes straight into the draft, with no `readonly`.
- The destination arm is the pattern to follow: `value: escaped.unwrap_or(value)`, `readonly: text_read_only`, and an
  `oninput` that ignores input while read-only. The hint `p.templates-value-readonly` is rendered for any field whose
  `text_read_only` is set.
- `e2e/tests/templates.spec.ts` injects older templates through `page.route` on the list endpoint because a PUT would
  refuse them.
- The save-as-template panel (`crates/farhelm-ui/src/list/save_template.rs`) already shows the session name escaped via
  `display_peer`; it is not part of this change.

## Outline

One PR: the `Field::Name` change, its docstrings, the unit and browser tests, the adjusted deletion test, the changelog
fragment, and the TODO.md removal.

### Validation

Per `.agents/validation.md`, choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the `farhelm-ui` templates unit tests through the recorder, the changed
`templates.spec.ts` tests on Chromium and WebKit through the recorder (with the builds they need), the test-sleep
checker, `python3 releasing/check-changelog.py format`, and `dprint check`.

## Fallbacks

Departures that need the scope reassessment (examples, not a blacklist): making the escaped name editable in place,
changing the save-as-template panel, or changing the save refusal itself.

Decisions to log: the exact adjustment to the deletion test.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-template-name-escape-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
("Sub-agents this file requires") requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from the no-delegation demand and run as that file says, not through galaxy-brain.

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
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/template-name-escape/<nn>-<short-name>` (for example `plan/template-name-escape/01-escape-name`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `fix`. Add a changelog fragment (`kind: fixed`) per root
  `AGENTS.md`.
- The last PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing one plan, steps
11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
