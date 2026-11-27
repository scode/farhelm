# Report: the default drift check runs real turns for Cursor and OpenCode and checks where Codex runs commands

## What this was about

The real-agent drift tests are the opt-in suite you run to see whether the latest agent CLIs still work with Farhelm.
Its default category, `base`, had two gaps the TODO entry named:

- For Cursor and OpenCode it only checked that the CLI starts: it stopped at their trust question or composer, so a
  release that broke typing a prompt and getting an answer under Farhelm's terminal host would still have passed.
- Nothing checked against a real Codex that the shell commands Codex runs stay inside the session, which is what lets
  Stop and Delete end them. Farhelm launches Codex with `--no-daemon` so that Codex does not hand them to its shared
  background server.

The entry also asked for two things the plan found already covered, so nothing new was added for them: that Codex's hook
reports come from inside the session (the supervisor already refuses a Codex report whose process ancestry does not
reach the session's terminal, and the `base/codex` turns require accepted reports), and that Codex starts cleanly with
`--no-daemon` when hooks are off (the `capture_timing_hooks` drift category checks exactly that; it passed live here,
see Checks). Both are now noted in the drift docs.

When the plan was written you decided: run one real turn for Cursor and OpenCode in `base`; put the Codex check in
`base/codex` rather than a new category; drop the TODO's Cursor "Agent's own setting" sub-item, since that mode no
longer exists; share code as much as possible; and, if a part got disproportionately complicated, leave it out with a
follow-up entry.

What the PRs do:

- Cursor's and OpenCode's `base` cases answer a workspace trust question if one appears, send one short no-tools prompt
  and wait for its answer on screen. Getting Cursor 2026.11.22 through needed two fixes to the shared screen reading:
  its trust dialog is drawn in a box with a `▶` selection mark, and it leaves the answered box on screen above its
  composer. Both fixes are in the trust handling every category shares (the named conversation category used the same
  trust answering, and the model selection category now uses the same rule for "the trust dialog is still waiting"), but
  only the `base` cases of Codex, Cursor and OpenCode were run live (see Checks).
- A vendor account whose usage no longer covers the model now reads as COULD NOT RUN before the first successful turn,
  like a missing login, instead of FAILED.
- `base/codex` also starts a second session, checks that Codex's process carries `--no-daemon`, has Codex run a small
  script that records its own process ancestry, and requires the session's Codex process among the ancestors with no
  detached server in between. The script runs through Codex's shell mode (`!` in its composer), which needs no model
  turn.
- The prompt and reply helpers moved to the shared drift code, the drift docs describe the new behavior, and the TODO
  entry is removed.

## Things you should know

- **The Codex check does not prove that `--no-daemon` alone keeps commands in the session.** It runs Farhelm's default
  Codex launch, and that launch's hook arguments also keep Codex off its shared server, as SPEC_impl.md ("Codex without
  the shared server") notes. So a Codex that ignored `--no-daemon` would still pass; only the argument's presence is
  checked. The plan asked for a throwaway negative control without `--no-daemon`; I ran one, but it kept the hook
  arguments, so it could not tell anything apart either. A check of `--no-daemon` alone needs a launch with hooks off
  and no other configuration overrides, which means answering Codex's own trust dialog, which the drift tests cannot do
  today. I applied your complexity fallback and added a Plan follow-ups entry in TODO.md describing it.
- **Cursor's answer reading is unverified live.** Every live Cursor run after the trust-dialog fixes reached the turn
  and submitted the prompt, then the account answered "GPT-5.6 Luna is not included in your weekly usage" for the
  default model, so the case reports COULD NOT RUN. Whether the test reads Cursor's rendered answer correctly will only
  show once that account has usage again. I did not switch Cursor's drift model to one the account covers: every
  category shares that setting and the plan named no model.
- **The Codex check runs its script through shell mode because the model is unreliable on the test machine.** Asked to
  run the script, Codex's model twice followed user-level instructions on the test machine first, a step that failed
  there, and then declined or chained the command so it never ran. Shell mode skips the model and costs no vendor turn.
  The check still speaks for the model's commands only because a shell-mode command runs in the same Codex process as
  theirs (without their sandbox and approval step), so it would go to a shared server just as theirs would. That comes
  from reading Codex 0.163.0's source, not from a live check; the live runs used 0.172.0. The Plan follow-ups entry
  includes confirming it.
- **During the negative control I started a Codex shared server in a private Codex home and stopped it.** It never
  touched the server state in your real Codex home. Its helper process survived Codex's own stop command and I ended it;
  the real Codex package directory was unchanged.

## Open questions and possible follow-ups

- The Plan follow-ups entry "Drift tests: isolate Codex's `--no-daemon` in the process-tree check" (see above), which
  also covers confirming where a shell-mode command runs.
- Run the Cursor `base` case once the account's usage resets, to confirm its answer reading.
- Run the drift suite more broadly (`base` for the other harnesses, and the named conversation and model selection
  categories), since #5167 changed how all of them answer trust questions and when they report a missing prerequisite.
  Each run spends vendor turns, which the plan authorized only for the three harnesses above.
- Unrelated to this plan: `cargo clippy -D warnings` on main fails in the helm crate, because a settings method from the
  landed work that lets an agent tell the user it is blocked (#5125, #5135) is used only by tests.

## PRs

- #5167 test: run a real turn for Cursor and OpenCode in the base drift case (the shared helpers, Cursor's trust dialog,
  the usage-limit verdict, the docs).
- #5168 test: check that Codex's commands stay in the session's process tree (the Codex check, its unit tests, the docs,
  the follow-up entry, the TODO removal).

## Checks

Run during this plan, through the test recorder unless noted:

- Run `90bd06b4`, `cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`: 49 passed, on the
  final stack. These are the drift module's ordinary tests, among them the new ones: Cursor's boxed trust dialog
  answered through its border, a composer below an answered trust box counting as ready while a dialog still showing its
  selection does not, the usage refusal reading as COULD NOT RUN only before a turn, and the Codex check's ancestry
  parsing and verdict (matching by pid and start time, a detached or self-started server rejected, a broken or short
  chain an error). Run `590349c6` was the same selection on #5167 alone (47 passed).
- Runs `c887e99e` and `c7e4d4d8`, `python3 -B scripts/test-run-harness-drift.py`: the drift runner's own tests, OK.
- Live drift runs, `python3 -B scripts/run-harness-drift.py --no-upgrade --category base --harness <name>` (the runner
  records each case):
  - OpenCode 1.19.6: PASSED (`3b0cf966`, `10f0eb03`, `0e2d2c54`, the last on #5167's final state).
  - Cursor 2026.11.22: COULD NOT RUN for the usage limit after submitting the turn (`f88060b0`, `677c6caf`). Earlier
    FAILED runs `705c6a3e` and `950de5cd` were the trust dialog problems above, and `af4adaaa` the usage refusal before
    it was classified; all three are kept.
  - Codex 0.172.0: PASSED on #5167 (`4a665f6d`), and with the final shell-mode check on #5168 (`03415104`, `e65e5df1`,
    `1f85a53e`, and `532dce05` on the final code). Earlier FAILED runs while the check still prompted the model are
    kept: the model declining (`69c27ff8`, `b928c382`), a prompt too long for the composer (`28381a61`, `8416a5c5`) and
    a shell-mode timing problem since fixed (`d7af8ad2`).
  - `--category capture_timing_hooks --harness codex`: PASSED (`31d18405`).
  - The throwaway negative-control runs (`b9dc9662`, `1fcf9c3f`, `69f0012b`) used a temporary change that is not in the
    PRs; they showed nothing either way, for the reason above.
- Not through the recorder, as they run no tests: `cargo clippy -p farhelm --tests` (no warning in the changed crate),
  `cargo fmt --all -- --check`, the test sleep check, and `dprint check` on the docs and TODO.md: clean.

Run now, on the stack as it lands: `cargo check -p farhelm --tests`, clean.

Reused: everything above ran before the final rebase. What landed in between (#5160 launcher cleanup, #5161 the CLI's
hosts table, #5162 the file hover hint, and plan bookkeeping) touches none of the drift code.

Skipped: the other harnesses' `base` cases and the other drift categories, live. #5167's shared trust and refusal
changes reach them, and their ordinary (non-live) tests above pass, but live runs spend vendor turns and the plan
authorized them only for Codex, Cursor and OpenCode; a broader run is listed as a possible follow-up. Every non-drift
suite, since only drift test code, its docs and TODO.md changed.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort, and #5168 had a second one after its rework.

- #5167: no blocker. Its two should-fixes are applied: the "dialog still waiting" rule now checks the whole screen for a
  selected answer, and the model selection category uses the same rule. Of its five nits, four are applied; I left out a
  unit test pinning that the prompt's echo cannot read as its answer, since the answer check's existing tests already
  cover that shape. It also flagged Cursor's answer reading as unverified (above).
- #5168, first review: one blocker, that the check could not isolate `--no-daemon`; handled as described above, with the
  follow-up entry. Its should-fixes are applied: reject a server Codex started under itself, require a complete parent
  chain so a cut-short walk is an error rather than "outside", and add the follow-up entry. Both nits are applied.
- #5168, second review: no blocker. Its three should-fixes are applied: run the script with `exec`, so a login shell
  that does not replace itself cannot read as a detached server; describe shell mode and the YOLO launch accurately; and
  refuse to start when the "Shell mode" cue is already on screen. Its three nits are applied. These last fixes were not
  reviewed again; runs `90bd06b4` and `532dce05` ran after them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the command line's parsing changes (cli-flag-parsing-audit, #5163
and #5165). The only `farhelm` command the drift tests' sessions run is the hook command the agents call, and its
parsing is unaffected: its free-text option still accepts a value that starts with a dash, and the agent names it is
given never start with one. In the same round, before this plan, terminal-tab-shortcuts landed (#5164, #5166: Cmd+T and
Cmd+W for shell tabs, desktop app only), and after it drift-permission-footer (#5169: Guarded permission footers in the
drift tests). The two drift plans share only the drift docs page and TODO.md; both pages' changes are kept.
drift-permission-footer's Cursor case should get past Cursor's trust dialog thanks to this plan's fix to the shared
trust answering (see that plan's landing notes).

An independent reviewer and I read the change and found nothing to fix. The checks tell the outcomes apart: the answer a
turn waits for cannot be matched by the echo of the prompt, and the Codex ancestry check matches processes by id and
start time and refuses a chain that stops short. The tests change no environment variables of the test process and every
wait is bounded. Their own code writes nothing to the real home directory, though the extra Codex session uses the real
Codex login and adds a session to Codex's own history there, as every drift case does.

#### Fixes made while landing

- **A TODO.md conflict.** The round's plans each removed their own TODO entries, and this plan also adds a Plan
  follow-ups entry, so stacking them conflicted there. I kept main's TODO.md with each plan's own TODO change applied,
  this plan's follow-up entry included; no code changed.

#### What the landing made untrue in the report

Nothing changed by the landing itself, but one attribution in the report is wrong: its last open question blames the
clippy failure on main on the blocked-agent notification work (#5125, #5135). It came from the change that made the helm
poll each host's session list every 30 seconds (#5120), which left the old ntfy settings save method used only by tests.

#### Worth knowing

- **One reason in the report and the new TODO entry may be wrong.** The report says a `--no-daemon`-only check needs an
  answer to Codex's own trust dialog, "which the drift tests cannot do today", and the Plan follow-ups entry says a
  `--no-daemon`-only check needs an answer to Codex's workspace trust dialog, which "the drift trust answerer does not
  recognise today". The reviewer found that the captured screen of that dialog from an older Codex (0.159.0) does match
  the answerer's rules. The documented reason the tests trust Codex at launch instead is that its dialog can lose
  keystrokes. The reviewer did not check the newer dialog (0.172.0) the live runs met, so the reason may still hold
  there; whoever picks it up should check first.
- **The live runs predate the landing.** As the report says, only the `base` cases of Codex, Cursor and OpenCode ran
  live, before the landing. The landing ran none of the real-agent cases, which spend vendor turns (see Checks).

#### Checks

Run once, on the round's three plans stacked on main as it stood when the landing started (terminal-tab-shortcuts, then
drift-base-isolation-and-turns, then drift-permission-footer), so each was tested with the others and with the earlier
changes above. The desktop plan got the desktop app's tests, screenshots and end-to-end smoke test. The drift plans'
real-agent cases are opt-in and spend vendor turns, so they were built and listed, not run, and the ordinary tests of
the drift code and of the supervisor's footer reading ran instead. The SSH test worker was unreachable, so the smoke ran
on a boat sandbox (an x86_64 Ubuntu 24.04 VM), and the rest ran locally:

- `cargo clippy --all-targets`, and `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent`
  (the build with the desktop app's test-automation hooks): each reported one warning and nothing else. That warning is
  the unused ntfy settings method in the helm, on main since #5120 and outside this round, which makes `-D warnings`
  fail, so these ran without it. `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (the desktop app's GUI
  crate, which does not depend on the helm's code): clean.
- The GUI crate's whole default test suite (`cargo nextest run -p farhelm-gui`), recorded run
  790d4ebc-9b6a-46d4-b81b-12cdbc9a08f6: 3,603 passed, none failed, and it skipped its 4 opt-in tests, among them the
  screenshot test the next item runs.
- The full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run 9be720dc-7a79-40ae-b77a-eec843eafb24:
  passed.
- On a boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  94e567ef-c5b4-4bbd-9a83-c0bac7dd24de: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps include Delete from a session's menu; none presses Cmd+T or Cmd+W.
- The supervisor's per-agent tests, the footer reader and its captured screens among them
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(/agent_kind::/)'`), recorded run
  7c2a5d10-f11a-46a9-a5d5-7ff23f798176: 112 passed, none failed.
- The ordinary tests of the real-agent test code, without the opt-in cases
  (`cargo nextest run -p farhelm --test e2e -E 'test(/real_agent_capture/)'`), recorded run
  56b688c2-75da-476f-baad-df4c39ca17a7: 50 passed, none failed, and no test reported that it skipped for a missing
  system service. Listing the opt-in drift cases
  (`cargo nextest list -p farhelm --test e2e --run-ignored only -E 'test(/real_agent_capture::drift::/)'`) built them
  and found 71.
- The docs website build (`bun run build` in `website/`): built. `python3 scripts/check-agents-md.py` for the drift
  tests' docs page, which the instruction files point to: passed.
- `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Not rerun: the drift plans' live runs against the real agents, described in their reports; the landing
ran none, since they spend vendor turns, so the drift plans have not run live together. Skipped: the rest of the Rust
workspace's tests, since nothing else changed; the other opt-in tests; and any run on a Mac, since none was available to
the landing. No Mac ran Cmd+T or Cmd+W: the Mac's rule is covered only by the tests that pass the platform in.
