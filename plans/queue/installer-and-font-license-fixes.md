# Execute three triage outcomes: installer file destinations, an incomplete latest-version answer, and the font license

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out three triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `installer-directory-target.md`
- `incomplete-latest-response-select-wrong-release.md`
- `ship-font-license-embedded-ui.md`

Two are in the Mac installer (the `curl | sh` script that installs and updates `~/Applications/Farhelm.app`), and one is
in what the web UI and the desktop app ship:

- When the installer updates an existing app in place, it renames each new file over the old one. If one of those
  destinations has become a folder, or a link to a folder (the app's main program, the `farhelm` forwarder the Terminal
  command points at, the icon, `Info.plist`, the record of which version is installed, or the installer's ownership
  record), the rename drops the new file inside that folder instead and still succeeds. The update then reports success
  while the app does not launch, and running the installer again does not repair it; a linked folder also receives the
  installer's write somewhere outside the app. It takes an app that was already damaged or edited by hand. Found by
  reading the code, not reproduced.
- An install that does not pin a version asks the release site which version is the latest. If that answer arrives cut
  short with a success status (a misbehaving proxy or CDN), a shortened tag such as `v1.2.1` from `v1.2.12` can still
  read as a valid version, and the installer then installs that older release and reports success. The desktop app's
  updater pins its version and is not affected. Found by reading the code, not reproduced.
- The web UI and the desktop app embed the JetBrains Mono Nerd Font, whose license (the SIL Open Font License) requires
  its full text to travel with distributed copies. The license file sits next to the fonts in the source tree but is not
  declared as an asset, so neither the web bundle the helm serves nor the desktop app's embedded UI carries it. The
  clipboard add-on's license notices appear to have the same gap. Found by reading the code; no release bundle was
  inspected.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the three entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- `scripts/test-install-sh.sh` covers a folder at the app's main program path and a link to an outside folder holding a
  sentinel, both refused with the sentinel intact and the installed version record unchanged; and a latest-version
  answer served with HTTP 200 and a body shorter than its `Content-Length`, refused with its message, nothing requested
  beyond `/latest`, and nothing created under the home directory. The existing `/latest` cases, the 404 message among
  them, pass unchanged.
- The web bundle and the desktop app's embedded UI both carry the full OFL text, and the clipboard add-on's notices if
  the same declaration covers them, at stable unhashed paths, and `scripts/check-desktop-assets.sh` passes.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-11):** the user asked for the whole review queue to be screened for findings
meeting all of these criteria: "it is CLEARLY a bug to be fixed. it's not ambiguous and doesn't need judgement that it
should be fixed given the nature of the product and its spec", "the fix seems reasonbly straight froward in the sense
that it won't add significant new complexity, require a re factor, etc.", and "its still relevant given current code vs
being stale due to code having changed"; and to triage that set as `fix code` "with an explicit complexity gate - it
must bounce back to me if during fixing it turns out to be more complicated and not fit the criteria". The decisions are
recorded in root `TRIAGE_OUTCOMES.md` (landed in #1896), one entry per feedback file, each with its own complexity gate
in its Completion criteria.

**The user's plan-time decisions (2026-10-11):** "let's use the planning system to schedule execution of the fixes".
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the installer and font
license group, so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes
only. Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires. The user is
upgrading the vendored xterm.js and its add-ons under `crates/farhelm-ui/assets/vendor` separately, with another agent:
this plan edits nothing under that directory.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me".

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; Claude Code shell commands, if you are Claude Code;
the rule against tests that modify the test process's own environment variables), `plans/AGENTS.md` (Executing one
plan), `review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for the installer test changes, and
`.agents/narrow-tests.md` (which records `scripts/test-install-sh.sh` as a monolithic pass with no per-scenario
selection).

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4.

- Installer file destinations: in `scripts/install.sh`, `replace_file` copies the new file beside its destination and
  then runs `mv -f` over it. `mv` onto a directory, or onto a symlink to one, moves the file inside instead of replacing
  it, on macOS and on Linux alike. The update path (`install_mode=update`) calls `replace_file` for six destinations:
  `Contents/MacOS/farhelm-desktop`, `Contents/Resources/Farhelm.icns`, the forwarder `Contents/MacOS/farhelm` (only when
  its bytes differ, and a folder never matches), `Contents/Versions/installed`, `Contents/Info.plist` and
  `Contents/.farhelm-installation`. The preflight that #1656 added (the `for folder in ...` loop in the branch where
  `current_record_is_ours` or `current_record_moved_here` holds) refuses linked or non-folder app folders before
  anything changes, but never looks at these file destinations. Extend it: once `install_mode` is decided and only when
  it is `update`, refuse with nothing changed if any of the six is a directory, testing with `[ -d ]`, which follows a
  symlink and so covers both a folder and a link to one. Keep the refusal in the same shape as the folder loop's (an
  `error` line naming the path and saying nothing was changed, then a line of advice, `exit 1`). The rebuild path is not
  affected: it builds a whole new bundle and swaps it in, so leave it alone. The ownership record is already guaranteed
  a regular file by `current_record_is_ours` and `current_record_moved_here`; list it anyway, since the ledger names it
  and the check costs nothing. Do not touch `replace_file` itself: by the time it runs, earlier steps have already
  changed the app, so a check there could not keep "nothing was changed". In `scripts/test-install-sh.sh`, extend the
  scenario "an update refuses to write through a symlinked folder of the app" (its `content_snapshot` and
  `links_to_sentinel` helpers already exist there) with two cases built on a 1.2.3 setup install and a `good-v2` 1.2.4
  update: a real folder at `Contents/MacOS/farhelm-desktop`, and that path replaced by a symlink to an outside folder
  holding a sentinel. Assert each premise before the update (it is a folder; it is a link whose target holds the
  sentinel), then that the update exits 1, the refusal names the path, the sentinel and the outside folder's contents
  are unchanged, nothing under `Applications` changed (bytes included), the Installed record still says 1.2.3, and the
  lock was released. Give the new cases a scenario comment with the why, as the neighbors have. The gate for this
  outcome: a preflight loop plus two fixtures. Needing to change the rebuild path, rewrite `replace_file`'s rename
  strategy, or refuse cases the ledger did not name to make the tests pass is a trip.
- Incomplete latest-version answer: in `scripts/install.sh`, section "3. Version", the unpinned branch runs
  `latest_reply=$(curl_get -fsS --max-filesize 256 -w ... "$ORIGIN/latest" 2>/dev/null || true)`, so curl's exit status
  is discarded and only the HTTP status from `-w` is checked. A body cut short under a 200 (curl exit 18) is accepted if
  what arrived parses as a version. Capture curl's exit status without tripping `set -eu` (for example `latest_rc=0`
  then `latest_reply=$(curl_get ...) || latest_rc=$?`, POSIX and BusyBox-ash safe), keep the HTTP status parsing as it
  is, and when the status is 200 but curl failed, clear the candidate and set a distinct `version_error` saying the
  answer from `$ORIGIN/latest` did not arrive complete (HTTP 200), with the same `FARHELM_VERSION=vX.Y.Z` advice the
  other messages give. The 404 message and the generic non-200 message stay byte-for-byte as they are, so the 404 branch
  keeps its precedence (with `-f`, curl also exits non-zero on a 404). Word the new message as "did not complete" rather
  than "cut short": a 200 that exceeded `--max-filesize` (curl exit 63) lands in the same branch. The pinned path
  (`FARHELM_VERSION` set) and the `SHA256SUMS` fetch are out of scope; a truncated checksum file already fails closed at
  verification. In `scripts/test-install-sh.sh`, the fixture server in `start_server` needs one new route: under a new
  scenario prefix (for example `/latest-short/`), `GET .../latest` answers 200 with a `Content-Length` for `v1.2.12`
  plus newline but writes only `v1.2.1` and returns, which closes the connection (Python's handler speaks HTTP/1.0 by
  default) and makes curl exit 18. Build a good release in that scenario's directory (`build_good_release` with 1.2.1)
  so the unfixed installer would install it and succeed, which is what makes the refusal a distinguishing observable
  rather than a shared failure. Add the case to the G2-G4 loop in "the latest release comes from the origin's /latest":
  refused, `/latest` requested, nothing but `/latest` requested, nothing created under HOME, and the new message. Add a
  premise check that the fixture really is a short 200, by asserting that a direct `curl` of the route reports status
  200 and a non-zero exit. Run that curl before the G2-G4 loop, beside the fixture setup, so its request is not counted
  in the case's "nothing but /latest" window; and give `latest-short` its own 1.2.1 release rather than adding it to the
  existing loop that builds 1.2.3 for each `latest-*` scenario. Confirm once that the new case fails against the unfixed
  `install.sh` and log that observation. The gate for this outcome: capturing one exit status, one message, one fixture
  route. Restructuring the version lookup, adding retries, or changing what other requests do is a trip.
- Font license: `crates/farhelm-ui/src/lib.rs`, inside `declare_assets!`, declares the two font files as
  `#[used] static` assets with `AssetOptions::builder().with_hash_suffix(false)`, and the web-links add-on's notice as
  `WEB_LINKS_LICENSES` the same way, with a comment explaining why a source-side text file proves nothing about
  distributed copies. `assets/fonts/OFL.txt` and `assets/vendor/addon-clipboard-LICENSES.txt` have no declaration. The
  ledger says to include the clipboard notices if the same declaration fixes them, and it does: it is the identical
  mechanism `WEB_LINKS_LICENSES` already uses. Add two declarations in that shape inside `declare_assets!` (so they join
  `all_assets()` and the parity check, and `asset_declarations_stay_inside_the_inventory_macro` keeps passing), each
  with a short comment pointing to the `WEB_LINKS_LICENSES` rationale rather than repeating it. The bundler flattens
  served paths to the basename, so they will be served at `/assets/OFL.txt` and `/assets/addon-clipboard-LICENSES.txt`;
  keep the existing file names. Leave `OFL.txt` byte-for-byte as it is: it is the license text. Update the font
  provenance paragraph and the `app.css` comment near the top that mentions `OFL.txt` so they say the notice ships. For
  the clipboard notice, put the "this notice ships" statement only in the new declaration's own comment: do not rewrite
  the `VENDOR_CLIPBOARD_JS` provenance block (beyond, at most, the one clause that says the notice only sits beside the
  source) and do not edit `addon-clipboard-LICENSES.txt`, because the separate xterm upgrade is likely to rewrite both.
  Before implementing this outcome, read every change under `crates/farhelm-ui/assets/vendor` and to the vendor
  declarations in `lib.rs` since 3f430cc4. If the upgrade renamed or replaced the clipboard notice file, declare the
  current one the same way; if the clipboard add-on or its notice is gone or no longer fits the same declaration, ship
  the OFL part and report the clipboard part back in the ledger's Execution field, as the ledger allows. No new Rust
  test: the precedent declaration has none, and the evidence that matters is the built bundle. After
  `scripts/check-desktop-assets.sh` builds the web dist, confirm that both files are in it, unhashed, byte-identical to
  their sources (`cmp`), and log that. The desktop app embeds that same dist and the helm embeds the web build's tree,
  so the parity check plus that inspection covers both. The gate for this outcome: two declarations and comment updates.
  If the bundler refuses or renames a `.txt` asset, or the parity check needs changing to accept it, that is a trip.
- The two installer outcomes touch different sections of the same two files and do not interact. The font license
  touches only `farhelm-ui`.
- Changelog: the PR is `fix:` (all three change what a user gets: a refused damaged update instead of false success, a
  refused incomplete version answer instead of a possible older release, and the license shipped with the app). One
  fragment with `kind: fixed`, written for someone running Farhelm, following `releasing/EDITORIAL_GUIDANCE.md`: the Mac
  installer now refuses to update an app whose main program or another file it replaces has become a folder or a link to
  one, instead of reporting success and leaving an app that does not start; an install without a pinned version refuses
  when the latest-version answer did not arrive complete, instead of possibly installing an older release; and the web
  UI and desktop app now include the license texts for the bundled terminal font and the clipboard add-on. Whether that
  is one fragment or two (the license item may read better on its own) is the executor's call; log it.

The complexity gate for these three is a small change each plus tests, as stated per outcome above.

### Validation

Follow root `AGENTS.md` "Finishing work". This PR changes a POSIX shell script, its bash test harness, one Rust file's
asset declarations and comments, a CSS comment and Markdown, so:

- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`.
- `bash scripts/test-install-sh.sh` through the recorder, for example
  `python3 scripts/record-test-run.py --kind development --selection 'install.sh scenarios' --concurrency 'one fixture server, sequential installs' --tmux none -- bash scripts/test-install-sh.sh`.
  It runs on Linux through its macOS `uname` shim. Apply `.agents/test-authoring.md` to the new cases.
- The Alpine/BusyBox run in `.github/workflows/ci.yml`'s `install-script` job is optional: the exit-status capture is
  plain POSIX that `sh -n` and shellcheck already cover. Log whether it ran.
- `scripts/test-uninstall.py` is macOS-only and the update path it drives does not change in a way this plan's fixtures
  miss; do not run it, and log it as unverifiable on this host.
- `cargo fmt --all -- --check` and `cargo clippy -p farhelm-ui --all-targets -- -D warnings`.
- A focused nextest run of the inventory fence test through the recorder (`--tmux none`), selecting
  `asset_declarations_stay_inside_the_inventory_macro` in `farhelm-ui`.
- `scripts/check-desktop-assets.sh` (needs dx at the pinned version, the `wasm32-unknown-unknown` target, and the
  desktop build's system packages; it builds into its own target directory), then the dist inspection described in the
  outline. If its prerequisites are missing on this host and cannot be installed without affecting anything outside the
  checkout or your scratch directory, the font outcome cannot be validated here. That is an environment gap, not a
  complexity-gate trip, but it is handled with the same mechanics so the installer outcomes still ship: leave the font
  outcome out of the PR, keep its queue item, set its Execution field to `pending` with "could not validate on the
  executing host" and the missing prerequisite named, and list it in the report's open questions.
- `python3 releasing/check-changelog.py format`, and `dprint check` on the changed Markdown.
- No Rust or browser test changes, so `scripts/check-test-sleeps.py` does not apply; no Playwright run, because no UI
  behavior changes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-installer-and-font-license-fixes-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
  `plan/installer-and-font-license-fixes/01-installer-font-license`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line,
and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you
found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape).
Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only when
every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be already fixed
on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit, per root
`AGENTS.md`, and remove its queue item.

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
the user's decisions. Because the PR changes tests and fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that the new destination preflight runs before
the installer changes anything and only on the in-place update path; that the exit-status capture cannot abort the
script under `set -eu` and stays POSIX (BusyBox ash included); that the 404 and generic `/latest` messages are
unchanged; that the new test cases would fail against the unfixed installer; that both license declarations use
`with_hash_suffix(false)` inside `declare_assets!`; and that the logged dist inspection (paths and `cmp` result) shows
them unhashed and byte-identical. The reviewer cannot build, so point it at the log rather than the bundle. Where you
disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in the log;
the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

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
alternatives considered: in particular where the destination preflight sits and why `replace_file` was left alone, the
wording of both new refusals, how the incomplete-answer fixture produces its short body, whether the clipboard notices
were included (and what changed under `assets/vendor` since 3f430cc4), the dist inspection result, whether the
Alpine/BusyBox check ran, the Conventional Commit type and changelog kind (and one fragment or two), every outcome
dropped by its complexity gate and why, and every review finding you decided not to follow. The user will ask for these
later.

### Unattended fallback

You never ask the maintainer anything in conversation, not even an optional question, a confirmation, or an offer of
more work: nobody is watching, and a question stalls the plan until someone looks. Every fork is either decided by you
and logged as a DECISION, or it is a block, handled per `plans/AGENTS.md` (Executing one plan, step 10). Anything the
maintainer should know, and any possible follow-up you noticed, goes into the plan's report.

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
