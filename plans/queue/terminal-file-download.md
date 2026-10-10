# File paths in a session's terminal can be downloaded from the session's host

Written against main at 2af378bd on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. It adds helm–supervisor protocol messages and bumps the protocol version;
if another change on main bumps it meanwhile, take the next number when rebasing.

## The goal

When text in a session's terminal looks like a file path (an agent saying "I wrote the file here"), hovering it marks it
as clickable, and clicking it downloads that file from the session's host to the user's machine.

Acceptance criteria:

- What counts as a path (D1, D2): an absolute path (`/srv/out/report.csv`), a home path (`~/out.csv`), a `file://` OSC 8
  hyperlink (Claude Code prints these for files; today they are silently dropped), and a relative path that contains a
  slash (`src/main.rs`, `./build/app`) or ends in a file extension (`notes.txt`, `report.pdf`). Plain words and
  version-like strings (`1.2.3`) do not count, and neither does any plain text that carries a URL scheme (`https://…`,
  `ftp://…`, `data:…`, `mailto:…`, and plain-text `file:///…` too) or starts with `//`. That keeps today's link behavior
  and the existing browser test that prints forbidden schemes and expects none of them to be clickable
  (`terminal-links.spec.ts`, "forbidden and malformed schemes are neither decorated nor clickable"). Only a `file://`
  OSC 8 hyperlink is a file link. A trailing `:line` or `:line:col` (`src/main.rs:42:7`) and trailing sentence
  punctuation or closing brackets are not part of the path. Plain http(s) links and http(s) OSC 8 links behave exactly
  as today.
- Where a path is looked up (D1): on the session's own host. `~` is that host's home directory for the session's user. A
  relative path is resolved against the session's directory (`SessionInfo.cwd`); a terminal tab starts there too
  (SPEC.md, Terminal-tab working directory). If a shell has since changed directory, the lookup can miss; that shows as
  "not found", which is accepted. A `file://` link's path is taken as is on the session's host.
- Hover (D3): the moment the pointer rests on something path-shaped, it is underlined and a tooltip shows a spinner and
  "checking…". When the host answers, the tooltip shows the host's name, the full resolved path, and the size of the
  file. Only then is it clickable (pointer cursor). Otherwise the tooltip says what is wrong, and clicking does nothing:
  "not found", "folder: not downloadable" (D4), "too large to download" with the size and the 100 MB limit (D5), "not
  readable", or that the host is not connected. Moving away cancels a pending check. There is no cache: each hover asks
  afresh, so a file an agent has just written is found. A `file://` OSC 8 link is underlined and given a pointer by
  xterm up front, which is accepted; its hover shows the same spinner and answer, and its click does nothing unless the
  answer was a downloadable file. Agent-controlled text is shown as data (the existing tooltip and `peer-value` handling
  of untrusted strings), never as markup.
- Click, in a browser: the file downloads and the browser saves it under the file's own base name.
- `file://` OSC 8 links need xterm's `allowNonHttpProtocols`, which passes every scheme to the terminal's link handler.
  Enabling it must come with an explicit scheme allowlist in that handler: http and https go to today's opener exactly
  as before, `file` goes to this feature, and every other scheme is ignored. In the desktop app a `javascript:` link
  reaching the existing opener would run script in the page, so this allowlist is a security requirement, with a browser
  test that a `javascript:` and a `data:` OSC 8 link do nothing.
- Click, in the desktop app (D6): the app always saves the file into the user's Downloads folder itself, never into the
  webview, picks a unique name if one is taken (`report (1).pdf`), and then shows an in-page message saying where it
  saved it. A failed or interrupted transfer leaves no partial file in Downloads.
- Every host, the local one included (D7). For the local host this reads the helm machine's own files; for the desktop
  app on the same machine it amounts to a copy into Downloads.
- Limits and failures: the 100 MB limit is checked before the transfer and enforced while it streams, so a file that
  grows past it mid-transfer fails cleanly. A file that vanished, a host that disconnected, or a read error mid-transfer
  shows an in-page message. Nothing is ever executed or opened.
- Trust (SPEC.md, Local authority and trust between hosts): any file the session's Unix account can read on that host is
  in reach, which is within the trust model since the helm already runs shells there. The text a click acts on comes
  from the agent, so the hover always shows the resolved host and path before a click can happen, and nothing downloads
  without that click. Only the helm may ask a supervisor for a file: a session-authenticated peer (an agent's `farhelm`
  command) is refused. The supervisor's restricted dispatcher already refuses every message it does not list, so this
  needs no new code, only a test.
- SPEC.md's Terminal experience gains a bullet describing the feature, its limits and the trust point above.
  SPEC_impl.md records the protocol messages and version, the helm endpoints, the limit, the scheme allowlist, and the
  desktop save path. The docs website's "Copy, paste, and links" section
  (`website/src/content/docs/docs/using/work-in-a-session.mdx`) describes it.
- Tests per Validation, including the path-recognition table (positives and negatives) as JS unit tests, the
  supervisor's resolve, stat and read (a file, a symlink to a file, a folder, a missing path, an unreadable file, a file
  over the limit, a file growing past it), the helm endpoints, a Rust end-to-end protocol test, and browser tests on the
  two-host stack for hover (spinner, then path and size), a download from the local and the remote host (through
  Playwright's download event), and the refusals. The desktop save path is tested with the Downloads directory injected,
  never by changing the test process's environment.
- The last code PR removes the TODO.md entry "Download files named in the terminal." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Download files named in the terminal. When text in a session's terminal looks like a file path (an agent saying 'I
wrote the file here'), hovering it should mark it as clickable, and clicking it should download that file from the
session's host to the user's machine. Which text counts as a path, how relative paths resolve, and what happens for a
missing file, a directory, or a very large file are to be decided when this is picked up."

**The user's decisions (2026-10-09):**

- D1. Full paths, `~` paths, `file://` links, and relative paths resolved against the session's directory, accepting
  that a shell that changed directory makes a relative path miss.
- D2. A relative path needs a slash or a file extension to count.
- D3. Existence is checked on hover, in the maintainer's words: "on hover but show hover immediately, with a spinner
  indicating its being checked. then full path, size if it exists."
- D4. A folder is not downloadable; the hover says so.
- D5. Files over 100 MB are refused, because the simple browser save holds the whole file in memory first.
- D6. The desktop app always saves into the user's Downloads folder itself and shows where it went.
- D7. Every host, including the local one.
- D8. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- Two operations, both asked of the supervisor by the helm: a stat (resolve the path, report kind and size and the
  resolved absolute path) and a read (stream the file). The read mirrors only attachment upload's flow control, in
  reverse: a begin message, data frames on a channel with the same credit window (`UPLOAD_WINDOW_BYTES`), and an end or
  abort, so a slow client cannot make the helm buffer a whole file and terminal traffic on the same connection never
  waits behind a download. Do not copy upload's stall timeouts, admission cap, staging or commit step; SPEC.md's "Slow
  hosts" and "Healthy local filesystems" sections are why. Optionally, the begin reply carries the size read when the
  file was opened and the supervisor sends exactly that many bytes, which also enforces the limit. Both are new tagged
  variants, so the protocol version goes up.
- Two helm endpoints under the session (stat and download), authenticated like every other REST call (the bearer
  header), so the browser fetches the file and saves it from a blob. No cookie or URL ticket is introduced.
- The desktop save: the desktop app passes its Downloads directory to its embedded helm as an optional setting, and the
  download endpoint, asked to save rather than return the file, writes it there; a helm without the setting refuses that
  request. Reuse the attachment code's unique-name and write-then-rename logic for naming and for never leaving a
  partial file. The file never passes through the webview.
- Path recognition as a pure function in asset JS beside `terminal-links.js`, exported for `js-tests`, used by a new
  xterm link provider registered alongside the existing two; `file://` OSC 8 links handled through the existing
  `linkHandler` with the allowlist above.

**What the planner found (at 2af378bd).**

- Terminal: xterm.js 6.0.0 with `@xterm/addon-web-links` 0.12.0 (vendored; see `VENDOR_XTERM_JS` and
  `VENDOR_WEB_LINKS_JS` in `crates/farhelm-ui/src/lib.rs`). URLs open through `openTerminalUrl` in
  `crates/farhelm-ui/assets/terminal-links.js`, guarded by a selection check and `isPlainWebUrl`; OSC 8 links go through
  the terminal's `linkHandler`, which shows the target on hover and rejects non-http(s) targets (`allowNonHttpProtocols`
  is unset). No custom link provider exists. `e2e/tests/terminal-links.spec.ts` notes that Claude Code prints `file://`
  OSC 8 links.
- No file read exists anywhere. The nearest pieces: `BrowseDirectory`/`DirectoryListing` (directory names only), and
  attachment upload (`BeginUpload`, `UploadStarted`, data frames, `UploadAck` credit, `CommitUpload`, `AbortUpload`;
  `UPLOAD_CHUNK_BYTES` 256 KiB, `UPLOAD_WINDOW_BYTES` 4 MiB) in `crates/farhelm-helm/src/uploads.rs` and
  `crates/farhelm-supervisor/src/service/uploads.rs`. Framing is described at the top of
  `crates/farhelm-proto/src/lib.rs`; `PROTOCOL_VERSION` is 43, and a mismatch is refused at Hello. Session-authenticated
  peers go through a restricted dispatcher in the supervisor.
- The session's directory: `SessionInfo.cwd` (with `~` expanded) and `canonical_cwd`. The supervisor does not track a
  pane's current directory.
- Helm auth: `require_device_session` in the helm's `auth.rs` (bearer for REST, a WebSocket subprotocol for terminals).
  Webview-facing routes need the desktop CORS and preflight layering (`api_router` in the helm's `lib.rs`). The
  clipboard bridge (`/api/clipboard` and its native `ClipboardSink`, 404 elsewhere) is described in SPEC_impl.md.
- Tests: `e2e/start-stack.sh` boots a two-host fleet (a local supervisor and one reached over ssh to localhost);
  `terminal-links.spec.ts`, `terminal-attachments.spec.ts`, `terminal-multihost.spec.ts`;
  `crates/farhelm-ui/js-tests/terminal-links.test.js`; `crates/farhelm/tests/e2e/attachment_uploads.rs` (protocol-level,
  with a `SlowFs` fault hook) and `merged_hosts.rs`; the helm's `rest_harness.rs` and `http_contract_tests.rs`.

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation; browser specs on Chromium and
WebKit through the recorder; the JS harness; the desktop compile and test checks; the test-sleep check), Releases and
the changelog, Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` first), Desktop/web UI bug
triage, Testability (no tests that change the process environment), Sharing the machine, Agent scratch space, The live
install is off-limits. SPEC.md's "Upgrade compatibility and client scale" (a protocol bump is allowed; updating from
v0.23.0 must keep working, and the helm must still be able to update older supervisors). `.agents/test-authoring.md` for
any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: the supervisor can stat and stream a file for the helm

`feat:` with a `kind: none` changelog fragment (nothing a user can reach yet), unless you find a better type for a
change with no user-facing effect; log it. The protocol messages and version bump, the supervisor's path resolution
(home, session directory), stat and streamed read with the limit enforced while streaming, the refusal for
session-authenticated peers, and the helm's side of the protocol. Supervisor unit tests and a Rust end-to-end test.

### PR 2: the helm's endpoints and the desktop save

`feat:` with a `kind: none` fragment as above. The stat and download endpoints with their auth and desktop CORS
layering, and the desktop-only save into Downloads with unique names and no partial files. Helm REST tests and desktop
tests with an injected Downloads directory.

### PR 3: paths in the terminal

`feat:` with the user-facing changelog fragment. Path recognition, the link provider, `file://` OSC 8 handling, the
hover with spinner and its states, the scheme allowlist, the click in a browser and in the desktop app with the in-page
messages, SPEC.md, SPEC_impl.md and the docs page. JS unit tests and browser tests. Remove the TODO.md entry.

### Out of scope

Downloading folders (as archives or otherwise), uploading by drag and drop, opening or previewing files, tracking a
shell's current directory, a file browser, resumable or parallel downloads, and raising the limit.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the supervisor, helm and
end-to-end tests that cover the new paths through the recorder, `cargo check -p farhelm-ui --features desktop` and the
desktop Rust targets when the desktop save changes, `cargo check -p farhelm-desktop`,
`cd crates/farhelm-ui/js-tests && node --test`, the relevant Playwright specs (the new ones and
`terminal-links.spec.ts`) on Chromium and WebKit through the recorder after `cargo build` and the `dx` web build, the
test-sleep check, `cd website && bun install --frozen-lockfile && bun run build` when the docs page changes,
`dprint check`, and `python3 releasing/check-changelog.py format`. Try it yourself in a browser against the two-host
stack and, on Linux, in the desktop app, and describe what you saw in the report; say plainly that macOS was not tried
unless it was.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-terminal-file-download-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/terminal-file-download/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

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

Before implementing a substantial departure from the outline above (folder archives, a cookie or URL-ticket auth scheme,
a streaming save path that bypasses the browser's blob, tracking a pane's current directory, a native Save-as dialog;
these are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run
a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
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
alternatives considered: in particular the path-recognition rules and their edge cases, the protocol messages and the
version number taken, the endpoints' shape, how the desktop save names files and avoids partial files, and how
session-authenticated peers are refused, and every review finding you decided not to follow. The user will ask for these
later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

## Blocked

Blocked while landing on 2026-10-10 (claim 88c8ce).

### Two review findings to settle before landing

This plan lets you click a file path an agent printed in a terminal and download that file from the session's host,
after a hover that shows the host, full path and size. The landing review (nothing of this plan is on main; PRs #1769,
#1771 and #1773 are open) found authorization, path handling and the click requirement sound, and raised two points.

1. After the first download in a terminal, a status strip ("downloading…", then "saved to …" or "download failed: …")
   stays pinned over the terminal's bottom rows, usually the agent's input box or status line, until the terminal
   closes. Nothing removes it and clicks pass through it.
2. The desktop app saves downloaded files into the Downloads folder without the macOS quarantine flag a browser save
   would carry, so a file a remote host chose opens without the "downloaded from the internet" prompt. SPEC.md's
   security section does not mention this new path from a host to the user's own machine.

Since the plan was built, managed-checkout-trash landed with helm-to-supervisor protocol version 44, which this plan
also claims; the next round must move it to 45.
