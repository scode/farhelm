# Targeted docs screenshots

## What this was about

Checking a few screenshots across different docs pages used to require a separate app start for each page. When the
capture could not find a place to leave the pointer without a tooltip or highlight, the message gave no clue what was in
the way. The maintainer asked for a single targeted run and enough information to diagnose a failure from its log.

## Things you should know

A single `scripts/docs-screenshots.sh --local` run can now take repeated `--only` arguments. For example,
`--only start-a-session/new-button --only launch-templates` takes one screenshot from Start a session and all eligible
screenshots from Launch templates. The runner checks names before starting the app or clearing previews, and tries each
selected screenshot once in declaration order. A selected run clears its own old PNGs and removes the previous publish
record, so it stays a draft even if the files it replaces look identical. Published release pictures still require
`--recapture-release-shot` for corrections.

The sandbox runner can also take page and shot selectors. It keeps a page's screenshots together and starts the app once
per sandbox. If one sandbox fails, the others keep their reservations and shared source until their captures finish. The
output directory loses any previous publish record before the new screenshots arrive.

If automatic parking fails, the capture reports which spots had no visible area, which were inside a reacting element,
and why the bridge refused to hover the others. Actual hover refusals come first. The report shows up to twelve reasons
and counts the rest. The underlying capture still tries every eligible spot in its original order.

## Open questions and possible follow-ups

Two existing GUI harness tests fail on clean main because they expect Bash and Zsh in the staged launch choices and the
list of programs replaced by agent wrappers. The screenshot selection and parking changes do not touch those lists. A
follow-up should first confirm how staged Bash and Zsh launches are meant to work. If they should run real shells, teach
the scenario validator about shell launch choices and limit the wrapper check to agent programs. If staged runs should
imitate shells too, add explicit shell fixtures and test their behavior.

## PRs

Both PRs are open drafts ready for landing review. #4446 is based on main and #4447 is based on #4446.

- Target pages and individual screenshots in one capture. [PR #4446](https://github.com/scode/farhelm/pull/4446/changes)
- Report what prevented automatic pointer parking. [PR #4447](https://github.com/scode/farhelm/pull/4447/changes)

## Checks run, reused and skipped

- On the selection change, 28 docs screenshot tests passed (`78665b20-f323-4767-b3a7-e317064d213a`) and five local
  sandbox-runner unit tests passed (`192399a4-7613-41c0-ae36-9e2566c883dc`). Actual selector and release correction
  preflights, Deno typecheck, ShellCheck, formatting, whitespace and instruction checks passed. The last check reported
  an existing stale heading citation in the review archive.
- On the final parking source at `40a5c29c4a75`, all 12 focused capture tests passed
  (`19f8957d-c0e7-40d8-b406-6f6ef92c502f`). The README video harness passed six tests on parking source `b32fbacb9b32`
  (`e442cefc-833e-4a80-8a48-d1f752304a02`). The final change to reporting leaves its accepted-target behavior covered by
  the focused capture tests.
- The wider 94-case GUI harness run had two shell staging failures (`dbe0e8b3-f868-492a-b4cb-0f9eb66a136a`). Both failed
  the same way in a focused run on clean main `8e134f94f78b` (`d4c96319-ecec-4ddd-aa86-85117d9d06a3`).
- Main stayed at the plan base through the final check. Stand-in bridge tests exercised mixed selections, draft output
  boundaries, continuation after a failed shot and parking refusals without a live app. No live capture or sandbox
  launch was needed for those contracts. The plan changes capture tooling, so the Rust workspace suite was outside the
  selected checks.

## Review gate

Each draft passed its source review and commit-message cold read. Review led to a path-component guard before cleanup,
tests for stale draft output in a reused destination, waiting for all member results before cleanup, and parking
messages that preserve the useful end of long target names and hover refusals.
