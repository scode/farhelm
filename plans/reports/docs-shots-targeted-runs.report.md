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

### Landing

#### What landed with it

The plan's two PRs, #4446 and #4447, merged together with a landing fix, #4453. Nothing else landed in this round. Since
the executor based the plan, main had gained only a fix to the release workflow's list of archives, which shares no
files or behavior with the screenshot tooling. An independent reviewer and I read the plan's two PRs against the plan.
Neither of us found a problem in any of these:

- how a run reads its page and shot names;
- the refusal of unknown names before anything starts or is deleted;
- the cleanup of old pictures, which before cleared a whole page and now clears only the selected shots;
- the boat sandbox runner keeping a page's shots on one sandbox;
- the parking report's limit and order.

#### Fixes made while landing

All three went in as the third PR, [#4453](https://github.com/scode/farhelm/pull/4453/changes), which changes no product
code. I read it myself and had its commit message cold-read. Nobody else reviewed it: it is two lists, a test, an error
message and two sentences of documentation.

- The two failing GUI harness tests that the report leaves as a follow-up are fixed. The harness is the script that
  drives the app for the docs screenshots, the README pictures and the end-to-end smoke test. The shell-sessions plan,
  which I landed on 2026-11-20, added Bash and Zsh as launch choices, and both tests compare lists in the harness with
  the app's source. I did not run them at that landing, because shell-sessions changed no harness file. First, the
  scripted app states the harness starts from may now include a recent launch of Bash or Zsh. Second, the harness keeps
  replacing every agent program with a fake agent, so that no vendor's real program runs. It now leaves the two shells
  alone. A shell session started in one of those runs therefore runs the machine's own shell, with the run's private
  home folder, which holds nothing of yours.
- The report asked whether such shell sessions should run the real shells or fake ones. I chose the real shells: no
  current screenshot or test starts a shell session, a fake `bash` placed ahead of the real one would also catch every
  script inside a session that runs `bash`, and faking them would be a new mechanism nobody has asked for. One
  consequence: Ubuntu's default prompt shows the account and host name. The capture's leak check refuses any screenshot
  that draws either, so a future shot of a shell session would fail rather than publish them, but it would need a
  neutral prompt first. A change of mind is a follow-up.
- Naming a published release picture with `--only` is refused. The refusal said to use `--recapture-release-shot`
  instead, but adding that option beside the same `--only` is refused too: a published release picture can only be
  retaken in a full run or with the whole `--only release-notes` page. The message now says that.
- The boat sandbox guide now says that a boat screenshot run first clears the pictures it is about to replace in its
  output folder, and for a selected run the publish record. That happens even if the run then fails, so a failed run
  leaves those pictures missing until it is run again. This is how the plan built it, and only scratch output is
  affected.

#### Checks

The plan changes only the docs screenshot tooling, the app-driving harness that captures them, the boat sandbox runner,
their tests and the instructions that describe them, none of which ships. The landing ran the harness and tooling tests
locally and one live capture on a separate arm64 Linux test machine.

On the plan as delivered, stacked on main:

- `python3 -B scripts/test-docs-shots.py` (recorder run `a1b54104-6ff3-4fa5-a193-59daaf7626d1`): all 28 passed.
- `python3 -B scripts/test-gui-agent.py` (recorder run `f8a72983-6260-4595-baab-009c6ffb4dac`): 92 of 94 passed; the two
  failures are the shell-list tests that also fail on main, fixed below.
- `python3 -B scripts/test-readme-video.py` (recorder run `876b2505-a010-488a-b832-13ec382ba32f`): all 6 passed, since
  the README video uses the same capture.
- `scripts/docs-screenshots.sh --local` with `--only start-a-session/checkout-setup`, `--only launch-templates` and
  `--only start-a-session/checkout-setup` a second time, on the test machine (recorder run
  `c369a1f7-d0c5-4362-88ce-7f994634719f`): passed, seven pictures in one app start, the shot named twice taken once.
- `scripts/docs_shots.py --check-selection` against the real pages: a mix of a page and shots passed, and a frozen
  release shot, an unknown shot, an unknown page and `..` were each refused with a message naming them.
- `deno check` and `deno test -A` on `scripts/boat/boat-tests.ts`, `shellcheck` on the boat sandbox scripts and the
  capture commands, `bash -n`, `dprint check` and `python3 scripts/check-agents-md.py`: passed. These are the tooling's
  own static checks and tests, run directly rather than through the recorder.

After the landing fix (#4453):

- `python3 -B scripts/test-gui-agent.py` (recorder run `2e1f16e1-2299-499f-b2c0-4bba876d6e61`): all 94 passed.
- `python3 -B scripts/test-docs-shots.py` (recorder run `007f58f4-1b8e-42a7-9f83-5b3892edae40`): all 28 passed with the
  reworded refusal, and the real `--check-selection` shows it.
- `dprint check` and `python3 scripts/check-agents-md.py`: passed.

Reused from the plan as delivered: the live capture, the README video tests and the boat harness checks. The fix changes
only which programs a staged run leaves unwrapped (the two shells, which no capture starts), the names a staged recent
launch may use, one error message and documentation, none of which those runs go through.

Skipped: the Rust suites, the goldens and the end-to-end smoke, since nothing that builds the app changed; a boat run,
since the boat runner's changes are covered by its own tests; CI, which runs only on demand.

#### Things you should know that the report does not say

- The executor ran no live capture. The landing did, on the test machine. It named one Start a session screenshot that
  is not the first on its page, twice, plus the whole Launch templates page. It took one app start, captured exactly
  those seven pictures and listed each once. So a shot can be taken on its own, without the shots before it on its page.
- Nothing ran the boat path for real: no sandbox was started. Its tests cover how shots are grouped by page and what a
  run clears.
- Tests that read the app's source, like these two, fail whenever a change to the app's launch choices skips them. A
  landing that changes launch choices or agent programs should run the GUI harness tests even when no harness file
  changed. I will do that from now on.
- A comment in the docs screenshot publishing script still says "one-page capture" where any selected capture is now
  meant. What it describes is still right, so I left it.
- goldens-fast-and-targeted, the plan in flight that makes the GUI's golden screenshot comparisons faster and lets a run
  name single scenes, will meet small text conflicts in `TODO.md` and in the list of checks a full test run uses, where
  both plans edit neighboring lines. Its landing keeps both sides.
