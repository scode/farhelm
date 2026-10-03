# Install output layout: a macOS-only installer with short, formatted output

Written against main at fbc89316 on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan has no dependency on another plan.

## The goal

`scripts/install.sh` (run as `curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | sh`)
becomes an installer for exactly one case, a Mac installing the Farhelm desktop app, and tells the user what happened in
a few short, nicely formatted lines instead of a wall of conditional advice:

1. SPEC.md says the installer is macOS-only for now, and says explicitly that only the installer is limited: Linux
   remains supported for running a helm and for session hosts (supervisors), which the helm provisions over SSH itself.
2. On Linux the installer refuses at once with the message below, changing nothing.
3. The Linux installation code and its tests are deleted. `FARHELM_NO_APP_BUNDLE` and `FARHELM_INSTALL_DIR` are removed:
   the installer always installs to `~/.local/bin` and always builds `~/Applications/Farhelm.app`. `FARHELM_VERSION`
   stays.
4. A release too old to carry the Mac app is refused, instead of installing only the command-line tool.
5. The output is exactly the messages in "The messages" below.

Acceptance criteria: each case below prints exactly its text (colors and the hyperlink only on a terminal); the Linux
refusal changes nothing on disk; the installer and uninstall test suites pass in their macOS shape; the docs and the
finishing-work inventory in root `AGENTS.md` describe the new installer; the TODO entries named below are removed.

## Requirement sources

**The user's request:** "use planning system to plan: … install.sh output …", later "make sure we plan these as separate
plans". The TODO.md entry, verbatim as of fbc89316: "Make `install.sh`'s output easier to scan. The completion message
is a wall of text mixing installation results, restart instructions, and setup advice. Improve the layout and visual
hierarchy, possibly with color; details TBD."

**The user's decisions (2026-10-03):**

- U1. "let's update SPEC.md to say that the installer will temporarily only support the assumption that you are on a
  macos laptop and want to install the desktop. nothing else." On Linux the installer bails with a message saying it
  currently supports only macOS, that this will be fixed, and that a real user who wants Linux should open an issue,
  which will be prioritized, with the issues link (the user allowed rewording; the text is fixed below).
- U2. "only the installer is macOS-only. nothing else. And be explicit about that. Linux IS supported for helm and
  supervisors, the temporary un-supported state … is to simplify and limit scope before I "announce" the project."
- U3. Delete the Linux half of the installer (code and tests), not leave it dormant.
- U4. "yes remove the two you mentioned (simplify simplify simplify) and absolutely keep version selector": remove
  `FARHELM_NO_APP_BUNDLE` and `FARHELM_INSTALL_DIR`; keep `FARHELM_VERSION`, which the maintainer uses all the time and
  real users may be asked to use for a release candidate or special tag.
- U5. "we should tell the user the most user friendly and brief/concise message we can, not a bunch of "if this then
  that" crap", "nicely with some good formatting, modern terminal stuff like colors/emojis". The user reviewed every
  case of "The messages" below and approved it, including: the uninstall line uses the full path; a progress section
  with a per-download label and curl's progress bar; the "safe to cancel and run again" note; brew.sh as a clickable
  link; the uninstall line before the tmux warning; "renamed to … - farhelm installation still proceeded."
- U6. Updating while Farhelm runs stays allowed, with no extra wording in the installer: "do not complicate installer
  messages". The update message is written for the behavior a separate, later plan will add (an update applied when
  Farhelm next starts, Sparkle style), and is also true today: the user quits and reopens Farhelm to finish updating
  either way. Do not build any of that behavior here.
- U7. The TODO entry "Uninstall after a move when the installer skipped the app" is dropped: with the two options gone,
  what remains (a re-run with the app lock held) is covered by the existing "rerun the installer" refusals. Remove that
  entry in PR 1.
- U8. No Linux install procedure is written anywhere, and the docs website is out of scope.
- U9. Review gate: a fresh-context Opus 5.5 reviewer at high effort, no swarm. No-workhorse mode.

**Planner choices, shown to the user without objection:**

- P1. Refuse a release too old for the Mac app (U5 approved the message). Planning research found every release has a
  desktop archive, and v0.2.1 is the first whose archive carries `Farhelm.icns` (#310, 75258e81); the too-old releases
  are v0.1.0, v0.1.1, v0.2.0, v0.2.1-rc.1 and v0.2.1-rc.2. Implement it by turning the existing post-extraction
  `ICNS_STATE != staged` skip into this refusal, emitted before the lock is taken and anything is replaced (step 6 of
  `main`); no version comparator. The only disk effects before that point are creating `~/.local/bin` if it is missing
  and the staging directory the EXIT trap removes, so the guarantee is "nothing installed", not "nothing on disk". The
  message's cutoff is the real one: "Pick 0.2.1 or newer".
- P2. Progress output (the ⏳ line, labels and curl's bars) goes to stderr, where curl draws its bar, so it stays in
  order; the final report goes to stdout; the Linux refusal and errors go to stderr. Color and bold appear only when the
  stream they are written to is a terminal and `NO_COLOR` is unset or empty; curl's bar appears only when stderr is a
  terminal (otherwise downloads stay silent, as today). The brew.sh link is an OSC 8 hyperlink on a terminal and plain
  text otherwise; its visible text is the address either way. Emojis print in every mode.
- P3. Errors keep their current wording and go to stderr, with `❌` in front (red on a terminal).

**Binding repository constraints:**

- `scripts/install.sh` header: POSIX `sh`, one file wrapped in `{ … }` so a truncated `curl | sh` runs nothing, no bash
  features, helper variables prefixed with their function's initials, `set -eu` defaults for every variable read.
  `sh -n` and `shellcheck` on it, and `shellcheck` on `scripts/test-install-sh.sh`.
- `crates/farhelm-helm/src/provisioning/assets.rs` parses the installer's asset table (`# BEGIN/END ASSET TABLE`, flush
  left) and its brace wrapping, and `install_sh_asset_table_matches_release_archives_exactly` diffs it against
  `RELEASE_ARCHIVES`. Provisioning remote Linux hosts never runs the installer (the helm pushes its own payloads). Keep
  the table's Linux rows as data, unused by the installer's `TARGET` lookup, and leave the Rust test unchanged.
- SPEC.md requires that a successful run prints the uninstall command and reports any file it kept aside.
- Root `AGENTS.md` lists `scripts/test-install-sh.sh` and `scripts/test-uninstall.py` in the finishing-work inventory
  with what they cover; update those descriptions in the same change as the behavior.
- Every installer test invocation goes through `env -i` with an explicit environment; macOS-shaped runs use the existing
  `uname` shim on the Linux CI host.

## The messages

These are the approved texts, exactly as they appear (published for review at
https://snippets.scode.org/s/farhelm-installer-messages/, which shows the colors). Versions, paths and timestamps are
examples. `[green]`, `[yellow]`, `[red]`, `[cyan]`, `[dim]` and `[bold]` mark terminal styling only; nothing is styled
when the stream is not a terminal or `NO_COLOR` is set. Indentation is three spaces after the emoji column.

**While it works** (stderr):

```
⏳ [bold]Downloading Farhelm 1.2.3[/]
[dim]   If it looks stuck, it is safe to press Ctrl-C and run the same command again.[/]

   [bold][1/2][/] farhelm command-line tool
######################################################################## 100.0%
   [bold][2/2][/] Farhelm app
######################################################                     76.4%
```

The bar lines are curl's own `--progress-bar`, one per archive, redrawn in place. The `SHA256SUMS` fetch is not counted
or shown.

**Fresh install** (stdout):

```
✅ [green bold]Farhelm 1.2.3 is installed.[/]

   Open Farhelm from Spotlight or ~/Applications.

[dim]   To uninstall later, run:[/] [cyan]~/.local/bin/farhelm uninstall[/]
```

**Fresh install, tmux missing or older than the floor** (stdout; the floor is the installer's existing constant):

```
✅ [green bold]Farhelm 1.2.3 is installed.[/]

[dim]   To uninstall later, run:[/] [cyan]~/.local/bin/farhelm uninstall[/]

⚠️  [yellow bold]Farhelm needs tmux 3.7c or newer before it can start.[/]
   This Mac has none. Install it with Homebrew: [cyan]brew install tmux[/]
   No Homebrew yet? Install it first: [cyan]https://brew.sh/[/]

   Then open Farhelm from Spotlight or ~/Applications.
```

With a tmux older than the floor, the second warning line reads
`This Mac has tmux 3.4. Upgrade it with Homebrew: [cyan]brew upgrade tmux[/]` (the version as `tmux -V` reports it).

**Update** (an existing installation was replaced; stdout):

```
✅ [green bold]Farhelm 1.2.3 is ready.[/]

   Quit and reopen Farhelm to finish updating. Your sessions keep running.

[dim]   To uninstall later, run:[/] [cyan]~/.local/bin/farhelm uninstall[/]
```

On an update with tmux missing or old, the tmux block follows the uninstall line exactly as in the fresh case, with its
last line reading "Then quit and reopen Farhelm to finish updating."

**A file the installer did not install was in the way** (added after the "Open"/"Quit and reopen" line, before the
uninstall line, one per kept file):

```
ℹ️  ~/.local/bin/farhelm was not installed by this installer, so it was renamed
   to ~/.local/bin/farhelm.replaced-20261003T120000 - farhelm installation still
   proceeded.
```

**A release too old for this installer** (stderr, exit 1, nothing installed; see P1):

```
❌ [red bold]Farhelm 0.2.0 is too old for this installer: it has no Mac app.[/]
   Pick 0.2.1 or newer, or leave FARHELM_VERSION unset for the latest release.
```

**On Linux** (stderr, exit 1, before any download or change):

```
❌ [red bold]This installer only supports macOS for now.[/]
   Linux is supported for running a helm and session hosts; only this installer is
   limited, and that will be fixed. If you want to install on Linux, please open an
   issue and it will be prioritized: [cyan]https://github.com/scode/farhelm/issues[/]
```

Any other operating system gets the same refusal with its first line unchanged.

**Cases the approved texts do not cover, decided at planning:**

- The report's PATH advice (the "not on your PATH" block and its colon and newline variants) is dropped; the full-path
  uninstall line replaces it.
- A `tmux -V` the installer cannot parse uses the old-tmux variant with the raw text it printed ("This Mac has tmux
  <what it printed>. Upgrade it with Homebrew: brew upgrade tmux"), never "has none", since tmux is present.
- An Intel Mac keeps today's "no release build for Darwin x86_64" refusal wording, as an error (P3).
- An unset `HOME` is an ordinary error (P3) before anything else, since the install directory is `$HOME/.local/bin`; the
  bundle step's "HOME is not set" skip goes away.

## Implementation outline

Two PRs, in this order. Line numbers drift; find code by name.

### PR 1: the installer supports exactly a Mac installing the desktop app (`feat!:`)

- SPEC.md (U1, U2): where it describes installing and the installer, say the installer is macOS-only for now and
  installs the desktop app and command-line tool on a Mac, and say explicitly that this limits only the installer: Linux
  is supported for running a helm and session hosts. Passages found at planning (find them by content):
  - the Topology-area sentence "The machine running the helm keeps its own supported setup: `install.sh` followed by
    `farhelm helm setup` on Linux, and the desktop app on a Mac." It must stop promising the installer on Linux while
    still saying a Linux helm is supported; say the installer temporarily does not cover a Linux helm machine, and do
    not describe an alternative procedure (U8);
  - the `FARHELM_INSTALL_DIR` and app opt-out sentences in the installer section, and "(`~/.local/bin` or
    `FARHELM_INSTALL_DIR`)" elsewhere;
  - SPEC_impl.md's `FARHELM_NO_APP_BUNDLE` text and "a different `FARHELM_INSTALL_DIR`" in its moved-installation text;
  - Rust docstrings that cite `FARHELM_NO_APP_BUNDLE` as rationale (`crates/farhelm/src/uninstall/removal.rs`,
    `crates/farhelm/src/uninstall/ownership.rs`): reword so they do not describe a live option, without changing
    behavior.
- Linux refusal before anything else in `main` (after argument and environment checks that cannot change anything).
- Delete the Linux installation paths: target selection for Linux, Linux-only report text (the systemd restart line, the
  `farhelm helm setup` advice), and anything only Linux reaches. Delete `FARHELM_NO_APP_BUNDLE` and
  `FARHELM_INSTALL_DIR` handling, so the install directory is always `$HOME/.local/bin` and the app step always runs;
  keep the ownership records, journal, rollback and moved-here rule as they are otherwise.
- Refuse a too-old release (P1) with the message above, before any change on disk.
- Tests, `scripts/test-install-sh.sh`: Linux is its default shape today, and the shared scenarios (integrity gate, 404,
  checksum mismatch, malformed archives, version normalization, missing prerequisites, the redirect chain, the
  closing-message fixtures) run Linux-shaped and pass `FARHELM_INSTALL_DIR` on every call. Convert them to the macOS
  shape (the `uname` shim, darwin fixtures including the desktop archive and icon, the install directory at
  `$HOME/.local/bin` under the fixture home); do not delete them. Delete only what exists solely for Linux or for the
  two removed options. Add the Linux refusal (nothing written, exit 1, the message on stderr) and the too-old-release
  refusal. Keep the CI workflow's Alpine/BusyBox leg as a POSIX `sh` check, now driving macOS-shaped runs, and reword
  the `install-script` job's comments that justify it by Linux platform detection. Messages may still be today's in this
  PR, except the two new refusals.
- Tests, `scripts/test-uninstall.py`: it cannot run macOS-shaped on Linux (`farhelm uninstall` picks its platform at
  compile time, and a Linux build refuses a record carrying a desktop digest), and after this PR the installer refuses
  Linux. Make it macOS-only (on Linux it exits with a clear skip, or asserts only the installer's Linux refusal), and
  delete its custom-directory and no-bundle install paths and its Linux service-fixture case. Its coverage then comes
  from macOS: run `gh workflow run ci.yml --ref <bookmark> -f suite=uninstall-macos` for this PR and report the run. Say
  in the report that end-to-end Linux uninstall coverage of installer-made installations goes away with the Linux
  installer (U3).
- Docs: `docs/install_uninstall.md` (macOS-only installer, the two options gone; no Linux procedure, U8), README.md's
  install section, `crates/farhelm-desktop/README.md` if it names the options, and the finishing-work inventory entries
  in root `AGENTS.md` for `scripts/test-install-sh.sh` and `scripts/test-uninstall.py` (including its "Linux children
  use a fixture service-manager command" wording and its local command, which becomes macOS-only).
- Remove the TODO.md entry "Uninstall after a move when the installer skipped the app" (U7).
- Changelog fragment `kind: breaking` (the installer now installs only on macOS; the two options are gone).

### PR 2: the installer's output is short and formatted (`feat:`)

- Replace the report with "The messages" above, in exactly that text and order. Add small helpers for styling (inside
  the `{ … }` wrapping, POSIX `sh`), gated per P2; no `tput` dependency is needed for plain ANSI and OSC 8 escapes.
- Progress: the ⏳ section and `[n/2]` labels on stderr, and curl's `--progress-bar` for the two archive downloads only
  when stderr is a terminal (the existing `curl_get` wrapper takes the extra flag; keep `-f`, protocol pinning and
  timeouts). Non-terminal runs stay silent until the report.
- Errors: prefix `❌` (red on a terminal) per P3, wording unchanged.
- Tests: update `scripts/test-install-sh.sh` to the new texts (its checks are substring checks of stdout and stderr,
  captured to files, so they see the plain form); add a check that a non-terminal run with or without `NO_COLOR` has no
  escape sequences; if a terminal can be simulated portably on the CI host (for example with `script`), add one check
  that the styled form appears, otherwise log why not.
- Remove the TODO.md entry "Make `install.sh`'s output easier to scan". This is the plan's last PR.
- Changelog fragment `kind: changed`.

### What not to build

- No detection of a running Farhelm, no staging of updates, no restart guidance beyond the update message (U6).
- No Linux install procedure anywhere and no website changes (U8).
- No new options or environment variables, and no second output format.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-install-output-layout-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/install-output-layout/<nn>-<short-name>`.
- PR 1 then PR 2, one commit, bookmark and draft PR each. Within this run, if a PR needs correcting, restructure it
  rather than stacking a correction on top, and do not add code in PR 1 that PR 2 deletes.
- Conventional Commits; PR 1 is `feat!:`, PR 2 is `feat:`. Each adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression. Typical choices, not
a checklist:

- Both PRs: `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`,
  `bash scripts/test-install-sh.sh`, `dprint check` on changed Markdown.
- PR 1: the asset-table parity test in `crates/farhelm-helm/src/provisioning/assets.rs` (a targeted nextest selection
  through the recorder), the uninstall unit tests whose docstrings you touched, and the focused macOS uninstall CI suite
  (`gh workflow run ci.yml --ref <bookmark> -f suite=uninstall-macos`), which is now the only place
  `scripts/test-uninstall.py` runs.
- `python3 releasing/check-changelog.py format` for both fragments.
- Before delivering, run the installer once in a terminal-like setting if you can (for example under `script`, with the
  `uname` shim so it runs macOS-shaped) against the test fixture server and look at the output, so the report can say
  the styled form was seen, or say it was not.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of that PR's changes. The user
demands a fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot
reach that model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: that PR's section above, The goal, The messages, U1-U9 and
P1-P3. Ask the reviewer specifically to check POSIX `sh` portability, the `curl | sh` truncation guard, and that the
Linux refusal happens before anything is written. Include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires for test changes. Address what the reviewer finds before moving on, and log the DECISION where you
decline a finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (detecting whether Farhelm is running, staging
updates, a new installer option or environment variable, keeping Linux code paths, a second output mode; these are
examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the request, the user decisions above, this
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
alternatives considered: in particular how the too-old refusal is placed, the SPEC.md wording for the Linux helm
machine, which test scenarios were converted and which deleted, the styling helpers and their gating, how the tests
check styled output, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. The message texts are the user's: do not
reword them; if one cannot be produced as written (for example a terminal limitation), block with the reason. A material
scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision;
a review finding or a log entry is not authorization. Anything that needs a decision: record the concrete tradeoff and
block per `plans/AGENTS.md` (Executing one plan, step 10). Because the PRs form one linear stack, if PR 1 blocks, do not
build PR 2 on top of it.

## Done criterion

The plan is complete when the two draft PRs exist as one linear stack, each satisfies its section above, each has passed
the review gate, and the TODO.md entries named above are removed. Open, not merged: merging happens only after the
maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
