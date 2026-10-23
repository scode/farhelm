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
