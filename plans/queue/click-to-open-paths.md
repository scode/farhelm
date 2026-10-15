# Cmd-click to open a terminal file path, and a clearer file hover

Written against main at 507dbcbb on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Paths an agent prints in a session's terminal (plain text, or `file://` OSC 8 hyperlinks) can already be clicked to
download the file from the session's host. Hovering shows a disclosure: the host, the full resolved path, and the size
or a refusal. Clicking saves the file: the desktop app into Downloads under a unique name, a browser as an ordinary
download.

Two changes:

1. **Cmd-click to open, in the desktop app.** Cmd-click (Ctrl-click on non-macOS desktop builds) saves the file exactly
   as a click does, then opens the saved copy with the operating system's default app, as double-clicking it in Finder
   would. Only a small allowlist of file types opens (`.md`, `.txt`, `.html` to start); anything else is saved but not
   opened.
2. **A clearer hover**, on every surface: the design the maintainer picked from mockups (variant D below), with the size
   human-readable and color-coded by size.

### Acceptance criteria

**Opening (desktop app only)**

- Plain click is unchanged everywhere: it downloads.
- In the desktop app, Cmd-click on macOS (Ctrl-click on other platforms the desktop app builds for) on a path whose
  check succeeded saves the file as a click does, then opens the saved file with the default app. The status line at the
  top of the terminal says where it was saved and that it was opened, or why it was not.
- **Only allowlisted types are opened.** The allowlist is exactly `.md`, `.txt` and `.html` (the maintainer's choice:
  "let's just allow-list a small set we allow instead. say .md, .txt, .html to start with"). Match the saved file name's
  final extension case-insensitively; a name with no extension, or anything not on the list, is not opened. A refused
  file is still saved, and the status says it was saved but not opened because only those types open (the wording must
  not claim the file is a program: most refused files are not). Desktop saves deliberately carry no macOS quarantine
  flag (SPEC.md), so there is no Gatekeeper prompt to fall back on; the allowlist is the safety boundary. It is one
  small pure Rust function with its own unit tests, deciding on the saved file's name only (saved files are created
  0600, so an executable bit can never be seen there). Widening the list is a later maintainer decision, not something
  to do here.
- The hover tells the user in advance: in the desktop app, a file whose type would not be opened shows no open hint, or
  says ⌘-click saves only (wording is yours). Compute this in one place: the helm's file check (`files/stat`) returns
  whether a Cmd-click would open the file, using the same Rust function, and only when the desktop opener is present;
  the JavaScript does not keep its own copy of the list. The final decision is still made on the saved file at open
  time.
- A refused or failed open does not turn the save into a failure: the response still reports the saved path, plus why it
  was not opened, so the user never sees "download failed" for a file that is sitting in Downloads. An open request the
  helm cannot honor at all (no opener) is refused before any transfer starts.
- A browser never opens files. There, the hover shows no open hint, and Cmd-click behaves like a plain click (it
  downloads). A standalone (server) helm refuses an open request, as it refuses the native save today.
- Opening goes through the desktop app's native side, the way the existing link opener and Downloads directory are
  injected into the embedded helm (`crates/farhelm-ui/src/desktop.rs`, `run_embedded`; `AppState.link_opener` in the
  helm). Do not route file opening through `webbrowser`: it is compiled with its `hardened` feature on purpose so that
  nothing can open a local file through it, and that stays.
- Opening is bounded to the file this click just saved: never a path the page or an agent supplies directly.

**The hover (all surfaces): variant D, "tier stripe and file icon"**

The maintainer chose this from the mockup page https://snippets.scode.org/s/farhelm-file-hover-variants/ (variant D).
That page lives outside the repository, so the design is written out here:

- A colored left edge (about 3 px) on the hover box carries the size tier; the body text stays neutral.
- First line: a file-type icon from the bundled Nerd Font (a few kinds: text/code, PDF, image, archive, folder, generic
  file), then the file name in bright, semibold text.
- Second line, dimmed: `host:directory/` (the resolved path without the name). Together the two lines must still
  disclose the host and the full resolved path, wrapped rather than shortened, with invisible and format characters made
  visible exactly as today: the split is presentation only.
- Third line: the human-readable size (for example `4.8 KB`, `8.4 MB`, `87 MB`; decimal units, 1 KB = 1000 bytes, so
  sizes agree with the 100,000,000-byte limit and a refused file never reads as under 100 MB) in bold, colored by tier,
  then `· click to download · ⌘-click to open` (the open hint only in the desktop app and only for an allowlisted type,
  with the platform's modifier name).
- Size tiers: under 1 MB `--ok` (green); 1–10 MB neutral text with an `--info` (blue) edge; 10–100 MB `--warn` (amber);
  over the 100 MB limit `--danger` (red, refused). Use the app's existing tokens, not new colors.
- Refusals (too large, folder, not found, not readable, not a regular file) show the reason in `--danger` on the third
  line with a red edge. "Checking" shows the existing spinner and a neutral edge.
- The mockup page's other variants (0, A–C, E–H) were not chosen.

**Spec, docs and tests**

- SPEC.md: "Terminal experience" ("Nothing is opened or executed") and the maintainer-confirmed decision under "Local
  authority and trust between hosts" ("Farhelm never opens or executes the result") are updated to the new rule: only an
  explicit Cmd-click in the desktop app opens a saved file, with the default app, and only for the allowlisted types.
  Record it as a maintainer decision of 2026-10-14. SPEC_impl.md "Session-host file reads" describes the open request,
  the native opener, the allowlist, that a server helm refuses it, and that any signed-in device of the desktop's own
  helm can request an open, as it can a native save today.
- The user docs that describe downloads (`website/src/content/docs/docs/using/work-in-a-session.mdx`, and
  `website/src/content/docs/docs/how-it-works/security-model.md`) say what Cmd-click does. Read `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md` first. Do not refresh the docs screenshots (that is a separate procedure); say in the
  report that the `download-hover` docs shot now shows the old hover.
- Tests: JS unit tests (`crates/farhelm-ui/js-tests/terminal-files.test.js`) for the size formatting and tiers, the icon
  kinds, and that Cmd-click requests an open only in native mode; helm tests in `crates/farhelm-helm/src/downloads.rs`
  for the open request (refused by a server helm before any transfer, a non-allowlisted type saved but not opened and
  still reported as saved, the injected opener called with the saved path and nothing else); the allowlist function's
  own unit tests (case, no extension, double extensions such as `notes.txt.command`, names that only contain an allowed
  extension elsewhere); the browser specs that assert the hover's text (`e2e/tests/terminal-files.spec.ts` and any other
  the change breaks) updated and extended for the new layout.
- A changelog fragment (`kind: added`) per root `AGENTS.md`.
- The TODO.md entry ("Click to open agent-provided paths.") is removed in the last PR.

Out of scope: opening in the browser UI (in a tab or otherwise), an in-app viewer or preview, opening folders, "reveal
in Finder", a setting for the modifier, changing which paths are recognized, the 100 MB limit, and the open review
findings about file links in `review_feedback_queue/` (mouse-reporting drags, invisible-character disclosure, oversized
paths): those stay queued, but the report must say that the mouse-reporting drag finding
(`mouse-reporting-drag-activates-file-link.md`) now also covers an open, not only a download, when the drag is a
Cmd-drag.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Click to open agent-provided paths.** Extend click-to-download for paths agents provide to also support
> click-to-open. How and implementation details are TBD.

**The user's decisions (2026-10-14):**

- "Open" means the default app, in the desktop app only: "default-app (desktop only) but we need to settle how to pick
  which. maybe cmd-click to open, click to download, or the other way around."
- The gesture follows the terminal convention (iTerm2, VS Code, WezTerm): Cmd-click opens, plain click keeps
  downloading. With it: "lets mock up the dialog and improve it while we're here anyway. for example i'd like the size
  to be visually indicated better, like color code by size, and just generally make the hover a little nicer".
- From the mockups, variant D (tier stripe and file icon), with the size tiers above.
- Only an allowlist opens: first offered as refusing executables, then, after planning review showed a list of refused
  types leaks (what runs depends on the installed apps), the maintainer chose: "let's just allow-list a small set we
  allow instead. say .md, .txt, .html to start with." Anything else is saved but not opened. HTML was chosen knowing
  that the default browser runs scripts in a local HTML file.
- Review gate is a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm; no-workhorse mode, which
  `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Terminal experience" (what counts as a path, the hover disclosure before a
click, the 100 MB limit, the native save into Downloads with no partial file, the OSC scheme allowlist) and its
maintainer-confirmed "Local authority and trust between hosts" (the click authorizes one transfer; no quarantine flag;
link opening is bounded); SPEC_impl.md "Session-host file reads" (bearer auth with no cookie or URL ticket, native mode
identified by the desktop handoff secret, no webview fallback) and the xterm.js island section (file downloads do not
consult the terminal link-click preference); `crates/farhelm-ui/Cargo.toml`'s comment on `webbrowser`'s `hardened`
feature; root `AGENTS.md` "Talking to the user" for status wording.

**Planner proposals:** Cmd-click in a browser behaving as a click; the advance hint for refused types, computed by the
helm's file check; matching the extension case-insensitively; doing the open helm-side after the save, through an
injected opener; the file-type icon kinds; the PR shape.

## Outline

As found at planning time (find code by name; line numbers drift):

- Detection, hover and click live in `crates/farhelm-ui/assets/terminal-files.js` (`FileLinksAddon`, `hover`,
  `download`, the status text and `.terminal-file-target`), wired up in `crates/farhelm-ui/assets/terminal.js` (the OSC
  8 `linkHandler`'s `activate`, and the plain-text provider); styles in `crates/farhelm-ui/assets/app.css`. The addon
  already knows whether it runs natively (`native`). xterm passes the mouse event to both the OSC 8 `activate` and the
  plain-text link callback, but the plain-text callback ignores it today (`() => this.download(path)`), so add the
  parameter. On macOS use `metaKey` (Ctrl-click on macOS is the context-menu click); the platform check (`IS_MAC`) lives
  in `terminal.js` and has to be passed into the addon. The bundled Nerd Font is the page font, so the hover inherits it
  and the icons render.
- The native save is `POST /api/sessions/{id}/files/download` with `save_to_downloads: true`, handled by
  `crates/farhelm-helm/src/downloads.rs` (`save_download`, which publishes through `publish_no_clobber` in
  `crates/farhelm-supervisor/src/files.rs`), and returns the saved path. The smallest extension: an `open: true` flag on
  the same request, valid only with `save_to_downloads`, after which the helm checks the saved file's name with the
  allowlist and calls an injected file opener with the published path. A server helm has no opener and refuses before
  the transfer, like it refuses `save_to_downloads`. The `files/stat` response gains the "a Cmd-click would open this"
  answer, present only when the opener is.
- The opener is injected from `crates/farhelm-ui/src/desktop.rs` the way `link_opener` and `downloads_dir` are. How it
  opens (macOS `open`, Linux `xdg-open`, or an ordinary crate) is your choice; log the DECISION. The helm test harness
  copies `link_opener` and `downloads_dir` when it rebuilds its state; copy the new opener there too.
- The allowlist is a small pure function with its own tests.

Expected shape, as a suggestion: (1) `feat:` the hover redesign with its JS and browser tests; (2) `feat:` Cmd-click
open with the helm, desktop and JS changes, SPEC.md, SPEC_impl.md, the docs pages, the changelog fragment, and the
TODO.md removal. Reshape if review reads it more easily another way.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
both clippy runs, `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`, the helm download
tests and the new rule's tests through the recorder, `cd crates/farhelm-ui/js-tests && node --test`, the changed browser
specs on Chromium and WebKit through the recorder (builds first), `scripts/check-desktop-assets.sh` if asset files
change, the website build if docs pages change, `dprint check`, and `python3 releasing/check-changelog.py format`. Run
the test-sleep checker (`python -B scripts/check-test-sleeps.py`, per root `AGENTS.md`) since browser tests change. The
desktop app cannot be driven by the browser specs; if no automated check covers the native open end to end, say so in
the report as a check not run, and do not run the maintainer's own Farhelm to try it (root `AGENTS.md`, the live install
is off-limits).

## Fallbacks

Agreed fallbacks: if the bundled Nerd Font is not available where the hover renders, use the nearest glyph the page
already has, or drop the icon and keep the stripe, and log it. If an exact tier color reads poorly against the hover's
background, use the nearest existing token and log it.

Examples of departures that need the scope reassessment: a new endpoint instead of extending the native save, opening
without saving first, a browser-side open, a new auth mechanism, a new font or icon set, a preference, widening the
allowlist.

Decisions to log: the allowlist function's exact matching, how the opener opens, the modifier handling, the icon kinds
and glyphs, the status wording, the hover wording for refused types.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-click-to-open-paths-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/click-to-open-paths/<nn>-<short-name>` (for example `plan/click-to-open-paths/01-file-hover`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `feat`. Add a changelog fragment (`kind: added`) per root
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
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
