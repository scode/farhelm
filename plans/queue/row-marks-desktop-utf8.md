# Declare UTF-8 in the desktop app, so the bell row stops garbling its marks

Written against main at fd025c7b on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

In the desktop app, a session row that shows a notification bell draws garbage over the agent and permission marks to
the bell's left, with a stray "<"-like glyph before the bell. Fix it by making the desktop app declare UTF-8 for its
page and its stylesheet, so text the stylesheet generates is decoded as written.

Acceptance criteria:

- The desktop app's asset handler labels every text response it serves (the stylesheet among them) as UTF-8, and a test
  pins that for the stylesheet.
- The desktop window's page declares UTF-8 itself.
- The zero-width character in the bell overlay's stylesheet rule stays as it is (D2).
- SPEC_impl.md says, where it describes the desktop asset handler, that the desktop app declares UTF-8 and why.
- The last code PR removes the TODO.md entry "Row marks pile up when a session's notification bell appears."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Row marks pile up when a session's notification bell appears.
When a notification showed up for a session, its row in the session list drew the marks to the left of the bell (the
agent and permission marks, as far as a photo of the screen shows) on top of one another into an unreadable smudge, with
a stray "<" between them and the bell. Seen on the selected row. Find what makes the marks overlap when the bell is
added, and fix the row's layout so every mark keeps its own space."

**What the planner found (at fd025c7b; find code by name, line numbers drift).** The layout itself is not at fault; the
glyphs are mis-decoded text.

- `crates/farhelm-ui/assets/app.css` has `.session-bell-overlay::before { content: "\200b"; }`, a zero-width space that
  gives the bell overlay a text baseline. The dx asset build's minifier writes it out as the raw UTF-8 bytes `E2 80 8B`;
  it is the only non-ASCII sequence in the built stylesheet.
- Nothing tells the desktop window those bytes are UTF-8. Farhelm passes no custom index or head to dioxus-desktop
  (`crates/farhelm-ui/src/desktop.rs`, the `dioxus::desktop::Config` it builds), so the window uses dioxus-desktop's own
  `prod.index.html`/`dev.index.html`, which carry no `<meta charset>`, served as `text/html` with no charset. The
  desktop asset handler (`crates/farhelm-ui/src/desktop/assets.rs`) labels responses with `mime_guess`'s
  `essence_str()`, so the stylesheet goes out as bare `text/css` and every script as bare `text/javascript`. A
  stylesheet or script without a charset takes the document's encoding, and WebKit's fallback there is a
  locale-dependent legacy encoding, windows-1252 in the reported case. Farhelm sets no custom head or index anywhere.
- Decoded as windows-1252, `E2 80 8B` is "â€‹" (the planner checked with `iconv`). The overlay is right-aligned with its
  `::before` ahead of the bell, so "‹" (U+2039, which looks like "<") lands just left of the bell and "â€", at the
  overlay's 14px weight 600, lands on the 22px agent track of harness and permission marks. Only rows with a bell are
  affected; "selected" was probably just which row had one.
- The web UI's `index.html` declares `<meta charset="UTF-8">`, which is why the Playwright bell-position test in
  `e2e/tests/notifications.spec.ts` passes in Chromium and WebKit.
- dioxus-desktop 0.7.10's `Config::with_custom_head(String)` injects its argument just before `</head>`. In release
  builds (`prod.index.html`) that is well within the first 1024 bytes, where a `<meta charset>` is meant to sit; in
  debug builds (`dev.index.html`, which the desktop smoke builds) it lands about 5 KB in. WebKit probably still honors
  it there, but the stylesheet's own charset is what fixes the bell either way, so a debug window still reporting a
  legacy `document.characterSet` is not by itself a reason to replace the index.
- The vendored terminal library (`crates/farhelm-ui/assets/vendor/xterm.js`) also contains non-ASCII text (line-drawing
  characters, the non-breaking spaces it draws for underlined spaces), so it is mis-decoded in the desktop app today
  too. The fix changes that as a side effect; say so in the report.
- The bug arrived with the bell itself (#1622, "show session notifications as a bell on the sidebar row").

**The user's decisions (2026-10-08):**

- D1. Fix it by making the desktop app declare UTF-8 for its page and its stylesheet, with a test pinning it. (The
  planner explained that declaring UTF-8 is the root fix and that replacing the character would only be extra safety.)
- D2. Keep the zero-width character. Do not add an ASCII-only guard over the stylesheet.
- D3. Review gate: "gpt-6.1-sol high, no swarm."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** SPEC.md "Session list" (a row's first line, the bell between the agent label and the
time) and "Status" (the bell); SPEC_impl.md "GUI: Dioxus" (row layout, the bell's slot); root `AGENTS.md` (Finishing
work, including the desktop checks; Conventional Commits; Releases and the changelog; Sharing the machine with other
agents; Agent scratch space; The live install is off-limits), `docs/desktop-web-triage.md`, `plans/AGENTS.md`
(Executing), and `.agents/test-authoring.md` for test changes.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### First: confirm the cause

Before changing code, confirm in a real desktop window that the stylesheet is mis-decoded, using whatever route the
desktop smoke harness (`scripts/desktop-smoke.sh`, run per root `AGENTS.md`) or a WebKitGTK window under Xvfb makes
practical. No bell is needed: a temporary, uncommitted probe in a desktop build can read `document.characterSet` and the
`content` of the `.session-bell-overlay::before` rule from the loaded stylesheet. Then confirm the fix the same way. If
no practical route exists, rely on the evidence above and say so in the report; do not build a new desktop test harness
for this. If the window turns out to decode the stylesheet correctly, the hypothesis is wrong: block with what you
found.

### PR 1: declare UTF-8 in the desktop app

`fix:` with a changelog fragment (kind `fixed`: in the Mac app, a session row showing a notification bell no longer
draws garbled characters over its agent marks).

- The desktop asset handler appends `; charset=utf-8` to the content type of every `text/*` response (the window loads
  only CSS, JavaScript, which `mime_guess` names `text/javascript`, and woff2 fonts), and leaves binary types as they
  are. Two existing tests expect bare `text/javascript` and bare `text/css`; update them, and pin the stylesheet's
  charset in the hit test or a small dedicated test rather than in the percent-decoding test. The handler's docstring
  says its content types match the helm's web responses; rewrite that sentence, since the helm stays bare.
- The desktop `Config` passes `with_custom_head` a `<meta charset="utf-8">`, so the page itself is UTF-8 and anything
  that inherits its encoding follows.
- SPEC_impl.md: one or two sentences in the desktop asset handler's description saying the desktop app declares UTF-8 on
  its page and text responses, and that WebKit otherwise decodes them in a locale-dependent legacy encoding.
- Remove the TODO.md entry.

Out of scope: the bell's list opening over the row's time and menu toggle rather than past the sidebar edge; the helm's
web responses (the web page already declares UTF-8); replacing the zero-width character.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `cargo check -p farhelm-desktop`, the desktop Rust targets through
`scripts/record-test-run.py` (the `desktop Rust targets` selection), and the desktop smoke (`scripts/desktop-smoke.sh`
with the pinned tmux on PATH), since it is the only check that drives the real desktop window and its asset handler.
`python -B scripts/check-test-sleeps.py` (per `docs/test-sleep-check.md`) if a Rust test changed.
`python3 releasing/check-changelog.py format` for the fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-row-marks-desktop-utf8-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/row-marks-desktop-utf8/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a custom index.html for the desktop window, a
charset change to the helm's web responses, a new desktop test harness, replacing the zero-width character; these are
examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the cause was confirmed (or why it could not be), which content types get the
charset, and where the meta tag is injected, and every review finding you decided not to follow. The user will ask for
these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
