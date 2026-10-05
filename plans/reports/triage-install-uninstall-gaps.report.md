## What this was about

Three small gaps, found in review and triaged on 2026-10-05, on the paths that install, update or remove Farhelm on a
machine. You decided all three were worth fixing as they stood, with no spec change.

- **A remote host could write terminal control sequences into the helm's log.** While adding or updating a remote host,
  the helm asks it to enable linger (so its supervisor starts at boot, not only at login). When that fails, setup
  carries on and the helm logs the host's error output, and it logged that output unescaped. A remote host is not
  trusted, so its error output could contain escape sequences that redraw the terminal of whoever reads the log, or set
  their clipboard on terminals that allow that.
- **Uninstalling a host could skip its identity check.** Before uninstalling a connected host, the helm probes the
  host's address again and compares the installation identity the machine reports with the one on record, to be sure it
  reached the same machine whose sessions it just checked. It only compared when the host had an identity on record, so
  for a host connected without one, a machine now answering with an identity passed unchecked. The spec already required
  an exact match, a missing identity included.
- **The Mac installer could write through symlinked folders inside Farhelm.app.** An update in place checked only that
  `Contents/Versions` was a real folder. If the app, `Contents`, `MacOS`, `Resources`, or the new version's folder had
  been replaced with a symbolic link, the update would have written the new program or icon wherever the link pointed.
  The spec already forbids symlinks redirecting uninstall's removal. This needs a link someone made deliberately.

## Things you should know

- **The log fix uses the existing helper** that escapes and length-limits text from a host, as other host error text is
  already logged. The host's linger error in the log is now cut at about 1 KB, like the rest.
- **Uninstall now refuses** when the host has no identity on record and the machine reports one, with a message saying
  so. A host with no identity on record whose machine also reports none still uninstalls as before (the two match).
- **The installer refuses before changing anything**, naming the link, and asks you to replace it with a real folder or
  move the app aside. Two things go slightly beyond the triage entry's wording:
  - Reviewers pointed out that a plain file where one of those folders belongs (for example a file at the new version's
    folder) was not caught, and an update used to move that file aside and delete it. The installer now refuses that
    case the same way.
  - A symlinked `Versions` folder used to make the installer rebuild the whole app instead of updating in place, which
    did not write through the link. It now refuses too, because the triage entry lists `Versions` among the folders to
    check. An old-layout app that is replaced as a whole is not checked; that path only renames, never writes through.
- **The installer test proves the refusal changes nothing**, byte for byte, on both sides of each link. With the check
  removed, 28 of its checks fail across all six linked-folder cases, so the test can tell the fix from its absence.

## Open questions and possible follow-ups

None.

## PRs

- #1654 — the linger error is escaped before it is logged.
- #1655 — uninstall refuses when an unrecorded identity appears.
- #1656 — the Mac installer refuses to update through symlinked folders (or non-folders) inside Farhelm.app.

They are stacked in this order, each on the one before, and land bottom-up. Each removes its review item from the queue
and records its execution in the triage ledger.

## Checks

Run on this stack. The ids are the test recorder's run ids, kept with each run's full output.

- #1654: the helm's linger tests, run 3719572d-5016-4a02-86cd-12ba2898bcc3, 4 of 4 passed (including the new test that
  an escape sequence, a clipboard write and an 8-bit control all come out escaped).
- #1655: the helm's uninstall tests, run 27abdf41-cf21-4d73-8692-7a49a73c7844, 13 of 13 passed (including the new
  identityless-host test); the new test was run again after a review fix.
- #1656: `sh -n` and `shellcheck` on the installer and its test, and the installer test script (which drives the macOS
  installer on Linux): 514 of 514 checks passed; the same script with the new check disabled failed 28 checks.
- `cargo fmt --check`, Clippy on the helm with warnings as errors, the changelog format check, and `dprint check` on the
  changed Markdown.
- After rebasing onto the latest main nothing was re-run: what landed in between only touched the demo-video recorder,
  the docs screenshot publisher and the shared review-queue and triage bookkeeping files, none of which these changes
  use beyond their own bookkeeping entries.

Skipped: the full helm suite and every other battery. The two helm changes are each confined to one function, whose
tests ran; the installer change is confined to one check in the installer script, which its own test script covers in
full.

## Review gate

Two fresh reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort.

- #1654: no findings from gpt-6-astra; Opus found it correct and suggested also testing an 8-bit control character and
  making the test's description say it checks the logged text, both done.
- #1655: no findings from gpt-6-astra; Opus found it correct and pointed out a test assertion that could not fail, now
  replaced with one that can.
- #1656: both found the installer change correct but the test too weak: it compared only file names, not contents, so an
  overwritten program behind a link would have gone unnoticed. Now it compares contents on both sides of the link.
  gpt-6-astra also found the plain-file case above, now refused. Opus's smaller points (a misleading test helper, an
  unreachable branch, the changelog wording) were fixed.
