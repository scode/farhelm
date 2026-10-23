## What this was about

Four findings from the 2026-10-22 triage about uninstalling Farhelm from a host through the hosts panel, plus one about
host planning. Uninstall deletes the host's Farhelm installation folder, stops its supervisor and promises to keep the
host's Farhelm data folder; each finding was a way it could delete what it promised to keep, stop sessions it never
checked, fail halfway, or hold up other hosts. All were found by reading the code; none has been reported by a user. You
decided each on the triage page, and on 2026-10-23, after I blocked on the first one for exceeding its 100-line size
gate, you chose to build it and raised this plan's cap to about 700 changed lines.

The five PRs, by what they protect:

- **Data reachable inside the install through another path to the home (your 2026-10-23 answer).** On a host whose home
  folder can be reached under two paths, uninstall's check that the data folder is not inside the folder it deletes
  compared path spellings and could miss it. The host now also checks, for the data path and each folder above it,
  whether it is the same folder as the one uninstall would delete, and uninstall refuses if so.
- **A mount inside the install folder.** The deletion followed into anything mounted inside the folder, such as a second
  view of the home, deleting those files too. Uninstall now refuses while anything is mounted on or inside the folder,
  checked when planning and again in the command that deletes it.
- **A retry while Farhelm cannot connect.** A retried uninstall without a connection assumed the supervisor was gone.
  With a custom command name that no longer works, it could stop a running supervisor and its sessions unchecked. It now
  refuses while the host's supervisor service still runs, and the refusal tells the user how to stop it themselves.
- **A data path with control characters.** Such a path passed planning and confirmation and failed only after the
  supervisor had been disabled, leaving a half-finished uninstall. Planning now runs the same check and refuses before
  the user confirms.
- **Output a helper keeps open.** Setup, update and removal waited indefinitely for output that a program ssh started
  kept open, while holding one of the helm's four planning slots, so a few such waits could stop every host's setup,
  update and removal. They now stop waiting ten seconds after the command finishes and report it as failed.

## Things you should know

- **Order of the stack.** The home-alias refusal is the first triage outcome but sits on top of the stack, built after
  your answer. Putting it underneath would have rewritten four PRs that had already passed review, and the outcomes are
  independent refusals.
- **The alias refusal closes one layout the mount check does not.** The triage's own example (a mount below the install
  folder, visible through only one path to the home) is already refused by the mount check. The layout only the new
  check catches is a mount on a folder above the install folder, visible through only one path to the home; its comments
  name that case so nobody removes it as redundant. Like the mount check, it also refuses a few layouts that would in
  fact be safe, as SPEC.md's rule for such layouts allows.
- **The mount check sends only the relevant mounts.** A first version sent the host's whole mount table to the helm,
  which on a busy container host could exceed the helm's 64 KiB limit on a host command's output and make uninstall
  unusable there; the host now filters it. A host whose mount table cannot be read is refused, not treated as having no
  mounts.
- **Ten seconds, not two, for lingering output.** The connection check before planning already gave up after two seconds
  and carried on. Here giving up fails the step, on steps every setup takes, and OpenSSH's connection sharing was seen
  to close output about half a second after a command ends, so two seconds left little margin under load. A helper that
  lingers longer than ten seconds now turns a delay into a failed step.
- **Two retry refusal details.** The offline retry treats a service that is starting or stopping as running, like the
  connected path. It does not cover a supervisor that exited while the terminal server holding its sessions kept
  running; triage left that out.
- **Size.** About 564 changed lines across the five PRs, under the 700-line cap you set. Before your answer, two PRs
  were already a little over the original 100-line gate on a strict count, and I treated them as within it: the mount
  check at 115 lines and the lingering-output fix at 112.
- **Docs.** SPEC.md and SPEC_impl.md now describe the offline retry condition and the alias check; BUGS.md's entry about
  an ssh helper keeping output open now covers only what remains (below). The changelog entries for the alias refusal
  and the lingering-output fix edit unreleased entries; the other three are new.

## Open questions and possible follow-ups

- **What still hangs.** Sending files to a host during setup or update can still wait indefinitely when the helm stops a
  stalled or failed transfer while a helper outside ssh's own process group holds its output. BUGS.md records it;
  bounding it was not part of this work.
- **An alternative for lingering output.** When only the error stream stays open after a command succeeds, Farhelm could
  carry on instead of failing, as the connection check does. That needs the output read so far to survive the cut-off; a
  possible follow-up if the ten-second failure turns out to bite.
- **Untested checks.** No test exercises the mount check that runs right before the deletion (it needs a real mount
  inside a folder that then gets deleted), nor the alias check against the exact mount-above layout (the test fixture
  cannot build real bind mounts and stands in with a data folder that does not exist yet). Both are covered by reasoning
  and by tests of the planning-time checks.
- **Smaller gaps left for size.** The offline-retry check at confirmation time has no separate test; a supervisor
  started by hand while the service is stopped still passes an offline retry; if the host cannot resolve its default
  data path, an earlier refusal still shows that path unescaped. SPEC.md's list of uninstall refusals does not name the
  new mount refusal (it already omits the existing data-folder refusal).

## The PRs

1. #2646 fix: refuse uninstall while something is mounted inside the install
2. #2648 fix: refuse an offline uninstall retry while the supervisor runs
3. #2650 fix: check the kept data path before confirming an uninstall
4. #2664 fix: stop host commands waiting on output a helper keeps open
5. #2668 fix: refuse uninstall when the kept data hides inside the install

Each triage outcome's `TRIAGE_OUTCOMES.md` Execution field names its change, bookmark and PR.

## Checks

Run on the final stack:

- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::)'`: run
  3022c2b1-8b5b-416e-9e72-97dce36f8b70, 231 of 231 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` for the workspace: pass.
- `python -B scripts/check-test-sleeps.py`: 314 test delays, none without a rationale.
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown files: pass.

Reused: none. Skipped: the tests that provision a real host over ssh with systemd (they compile with the changed calls
but need a real remote host); the installer and CentOS provisioning harnesses (no installer change); UI and browser
tests (no UI change). Nothing was run against this machine's own Farhelm installation.

## Review gate

Each PR passed a fresh-context Opus 5.5 review at high effort, and every finding that mattered was fixed before push:
the mount check's whole-table output (above), the alias check's comments naming the layout it alone closes, the
lingering-output test proving the bound and the ten-second margin, the data-path test reaching the check through the
route that actually reaches it, and accurate refusal wording. Findings not followed were left for size or scope and are
listed under follow-ups.

### Landing

Landed as #2646, #2648, #2650, #2664 and #2668, merged bottom-up, with the content the report describes and two docs
fixes described below. It was the first of two plans in this landing round, before per-host-last-folder.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained alias-in-add-host (adding a host that is already listed is refused, and the add can
carry an alias), which changes the same setup code in the helm. A fresh-context reviewer and I read the two against each
other, and against per-host-last-folder, the other plan in this round:

- They work on separate paths through that code: alias-in-add-host changes how a host is checked and registered when you
  add it, this plan changes the uninstall checks and how every setup step reads a command's output. Nothing in this
  plan's code or tests goes through the new check made when adding a host, and the new field this plan adds to the fake
  host used in the helm's tests is filled in where the alias tests build theirs too.
- The "alias" in this plan's home-folder check means a second path to the same home folder, which has nothing to do with
  a host's display alias.
- The browser tests' fake remote host still reports a running supervisor with its service present, so no browser test
  reaches the new refusal of an offline retry.
- Two changelog entries the plan edits belong to changes made after the last release, so editing them was right; the
  three new entries fix behavior that shipped.

#### Fixes made during the landing

- The "Manage hosts" and "Uninstall Farhelm" docs pages said that once a failed uninstall has removed the host's
  service, uninstalling again finishes even though the host no longer answers. #2648 now refuses that retry while the
  supervisor still runs, so both pages now say the retry finishes once the service is removed and the supervisor has
  stopped. In #2648.
- The "Manage hosts" page lists the less common setups uninstall refuses, as a closed list. The landing added the new
  case of a `~/.local/lib/farhelm` directory with something mounted on or inside it, in #2646. The list still leaves out
  two other new refusals, a data directory reachable inside the installation through a second path to the home
  directory, and a data path with control characters; see the follow-ups.

#### Possible follow-ups

- Found at landing: SPEC.md's list of uninstall refusals does not name the new mount refusal, and SPEC_impl.md does not
  describe the mount check at planning, its recheck just before deletion, or the planning-time check of the data path's
  characters (the report already notes the SPEC.md gap). A short spec update would make them agree with the code. The
  "Manage hosts" page's list of refused setups could name the home-alias and control-character refusals at the same
  time, or stop reading as a complete list.

#### Checks

This round landed two plans: the helm's uninstall and setup safeguards (uninstall-guards-oct22), and the session
launcher's per-host directory (per-host-last-folder). The first touches the same setup code as alias-in-add-host, which
landed on main just before this round, and neither executor's checks ran with alias-in-add-host on main. So this round
ran the whole Rust workspace, the launcher's browser specs and the provisioning browser spec on both plans stacked
together on this round's main. The first three items below ran before the landing's docs fixes to uninstall-guards-oct22
(three paragraphs on two docs pages); the website build ran again after them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the docs website build, the
  workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  slots, no retries, run `0ac84ed9-860d-47ec-ba61-0c1a1a4f516c`: 4325 passed, 40 skipped by design (helper entries that
  only run as child processes of other tests, and tests that need a real signed-in Claude or Codex). This includes the
  helm's setup and uninstall tests, and the tests that set up, update and uninstall Farhelm on this machine over ssh,
  which the uninstall-guards-oct22 report lists as skipped; they ran and passed here.
- `npx playwright test` in `e2e/` on Chromium and WebKit for the nine launcher spec files the per-host-last-folder
  executor ran (`yolo-guard`, `replace`, `terminal`, `destination-authority`, `quick-launch`, `sidebar`,
  `terminal-create-idempotency`, `clone`, `permission-switch`) and `provisioning`, three workers each with its own helm
  and supervisors, no retries, run `0c90ae2b-16a2-4053-95db-b34184859aed`: 779 passed, 1 failed. The failure was
  WebKit's "offer retirement releases only its owner: waiting behind another host" in `provisioning.spec.ts`: clicking a
  host menu's uninstall item kept finding it detached from the page until the test timed out. The test answers the
  uninstall requests itself, so the helm's uninstall planning these plans change never runs in it. Ten WebKit repeats on
  this stack (run `a50fb0eb-6a64-46bb-9850-427d9c965492`) failed once, and thirty on main without the plans, with main's
  own binaries and web build (run `0964225d-00ee-4deb-9daa-5c0227ae3bdf`), failed 7 times: six the same way, and one a
  step earlier, while the host menu was still opening. It is a failure main already has; it is recorded in FLAKES.md,
  with a Deflake entry in TODO.md, in draft PR #2673.
- Reused: none. Skipped: the rest of the browser suite (the launcher change is covered by its specs above, and the
  uninstall changes have no browser-side code), the doctests (no documentation example changed), the desktop smoke test,
  the native Mac app (not runnable here), the installer (unchanged) and the macOS-only installed uninstall suite (not
  runnable here; these changes are in the helm's uninstall from the hosts panel, not the installed uninstaller).

The report's checks ran the helm's provisioning unit tests on the plan before alias-in-add-host landed; this round ran
the whole Rust workspace and the provisioning browser spec with both on main. Nothing in the executor's report became
untrue at landing.
