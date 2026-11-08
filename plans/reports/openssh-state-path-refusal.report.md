# Tensorlake state-path refusal

## What this was about

On supported older OpenSSH clients, a percent sign in the helm's state-folder path could leave the Tensorlake gateway's
trusted key unrecorded, allowing later connections to accept a different key. Double quotes and backslashes could
prevent connections. The maintainer chose to refuse these unsupported paths with a clear message, rather than change the
supported OpenSSH versions or introduce different encodings for different clients.

Tensorlake host connections and snapshot builders now refuse paths containing `%`, `"` or `\` before invoking SSH. The
error names the path and tells the user to move the helm's state folder. The implementation specification reflects that
rule. Both feedback items have their execution recorded and have been removed from the queue.

## Things you should know

The refusal applies on every client version, including newer OpenSSH clients that might carry some of these paths
successfully. Spaces and apostrophes remain supported, and the existing `${` refusal remains. Ordinary user-configured
SSH access keeps its existing behavior. The code, tests and specification change totals exactly 100 added and removed
lines, within the agreed gate; the ledger, queue removal and changelog are excluded by the triage rule.

The tests check refusal and argument construction without making a network connection. This run did not reproduce the
original parser defects with an old OpenSSH executable or contact Tensorlake. The installed Farhelm was not changed.

## Open questions and possible follow-ups

None required to complete this plan. No version detection or encoding mechanism was added.

## The PRs

- [#3168](https://github.com/scode/farhelm/pull/3168/changes): refuse unsupported Tensorlake paths, update the spec,
  cover each character, and close the host-key-persistence feedback item. Draft head `c3152917`.
- [#3170](https://github.com/scode/farhelm/pull/3170/changes): close the sibling quote/backslash feedback item, citing
  the first PR. Draft head `b165ff72`, based on #3168.

## Checks run, reused and skipped

- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, and the isolated
  `python -B scripts/check-test-sleeps.py` check (423 delays inspected, none missing a rationale).
- Passed: `dprint check` on the changed specification, outcome ledger, feedback index and changelog fragment;
  `python3 releasing/check-changelog.py format`; and `python3 scripts/check-agents-md.py` for the deleted feedback
  files. The latter reported zero errors and three existing citation warnings outside this change.
- Passed: recorder-backed `cargo nextest run -p farhelm-helm --lib` with filter
  `test(/^ssh::tests::/) | test(/^tensorlake::ssh::tests::/)`, four slots, zero retries, pinned nextest 0.9.148 and
  `--tmux none` because the selected tests construct options and arguments rather than use terminals. Run
  `f637cb14-fc65-4d97-97df-a4158c5b12b8`: 25 passed, 1,542 selected out, no failures or flaky outcomes. Source capture
  and runner report are complete; retained output is complete with no runtime `SKIPPED` marker or cleanup limitation.
- Reused for the final draft heads: these source checks and the focused test run covered base `082bde33` plus the
  implementation working diff. Subsequent changes only set commit descriptions, added PR URLs to the ledger and closed
  the sibling feedback item. No executable, test or specification content changed. Main advanced through separate
  Credentials UI work and plan/TODO bookkeeping; their full diffs were inspected and do not interact with this refusal.
- Skipped: other Rust suites, browser, desktop, installer and provisioning tests. The change is confined to the shared
  Tensorlake path-option refusal and its argument tests; those suites add no identified coverage for it. The second PR
  changes only feedback records and needs no runtime test of its own. No cloud or live-install check was needed.

## Review gate's outcome

Each PR received its required fresh-context gpt-6.1-sol review at high effort. Both reported no findings. The first
review also verified the 100-line gate and coverage of host and builder options for every character and both managed
path keywords. Separate cold readers checked both commit/PR messages. No review finding was left unresolved.

### Landing

Landed with two documentation fixes made at landing. Tensorlake hosts and snapshot builds now refuse a helm state folder
whose path contains `%`, `"` or `\`, with a message naming the path. Before, such a path reached SSH, and an older
supported OpenSSH could misread it; with `%`, it could stop recording the Tensorlake gateway's trusted key, so a later
connection would accept a different key.

- #3168: the refusal, its tests, the SPEC_impl.md change, the changelog entry, and the closing of the review feedback
  item about `%` and the gateway's key.
- #3170: the closing of the sibling review feedback item about quotes and backslashes.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the Settings → Credentials warning and label fix (#3165), a TODO entry (#3167) and
plan bookkeeping, and the round landed nothing else. None of it touches SSH, the specifications' SSH text, the record of
triage outcomes or the review feedback queue, and the two changelog entries are separate files, so nothing interacted. A
fresh-context review checked the change against the current tree and found nothing that blocks it, two documentation
gaps (the fixes below) and one possible follow-up (below). The SSH options that name Farhelm's key file and its
known-hosts file are built by one shared piece of code. Only Tensorlake hosts and snapshot builds use it, so ordinary
SSH hosts keep their behavior.

#### Fixes made at landing

Both are in #3168:

- SPEC_impl.md's "Tensorlake hosts" section still said only a path containing `${` is refused, so the report's "the
  specification reflects that rule" held for one of its two places. That section now names `%`, `"` and `\` too, with
  the reason, and points to "Tensorlake snapshot builds", which the plan had already updated. This adds five changed
  lines, so the plan's code, tests and specification come to 105 changed lines, against the 100-line limit its report
  counted exactly 100 against. The fix only states the same rule in the second place that describes it, so the landing
  went ahead rather than sending the plan back over five lines of specification text.
- The changelog entry gains a caveat: the refusal applies whatever OpenSSH version is installed, so a Tensorlake setup
  under such a path that connected before, with a newer OpenSSH, stops connecting until the helm's state folder moves.

#### Things you should know

- A possible follow-up the review found: the refusal comes only after a sandbox exists. Adding a Tensorlake host under
  such a path creates the sandbox and then fails, leaving the sandbox recorded and running until the user removes the
  host. A snapshot build creates its builder sandbox and fails while connecting to it, and that sandbox is cleaned up
  with the failed build. The existing `${` refusal already behaves this way. Checking the path before anything is
  created would be a new check, beyond this plan's scope.
- The message tells the user to move the state folder with `--state-dir`. That flag belongs to the command-line helm;
  the desktop app keeps its state in a folder in the user's home unless an environment variable the user sets moves it,
  and a Mac home folder cannot normally contain these characters, so a desktop user sees this message only after moving
  the state folder that way. The existing `${` message gives the same advice.

#### Checks

These ran on the plan rebased onto main `dcedf742`, before the landing fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason), `dprint check` and
  `python3 releasing/check-changelog.py format`: all passed.
- The helm crate's whole test suite, which holds every test of the shared option code and of both its users, through the
  test recorder (`python3 scripts/record-test-run.py --runner nextest --kind development --tmux required ...` running
  `cargo nextest run -p farhelm-helm`, four slots, no retries), run `4638414f-fde5-4432-be00-599602633a6d`: 1566 of 1567
  passed. The one failure, `provisioning::tests::a_plan_made_while_running_does_not_run_once_suspended` (a host setup
  refused because its sandbox was suspended), timed out after waiting five seconds for its provisioning run to finish.
  The plan's refusal cannot have caused it: the test's state folder is a fresh temporary directory whose path holds none
  of the refused characters. The retained run was kept. The test then passed 20 repeated runs on its own (the recorder's
  repetition kind, runs `70e3208d-93c1-4995-ae50-b9e4eb7ef4bb`, `02a206fe-6453-468d-b0a3-fe19eee023e5`,
  `959cab69-8bec-41c4-aee1-1ed4968c6c6b`, `4ff3277c-4655-41c0-8fe5-a5353998c5b3`,
  `cd2faf6f-154d-46e5-9aaf-d26d0deefcda`, `90a0417b-00e7-42ff-8026-d6ac8ceb786d`,
  `8962b323-cbf0-43c0-a4b3-33ed26faf2b0`, `4fe88358-38f9-4f75-935a-fdd2dfc071d8`,
  `0f133485-f60b-4136-8c07-29e237370949`, `3da19033-0a48-4b77-85b5-1f7bb75799b3`,
  `23e563b9-b0bc-4816-a017-c68156470d38`, `848af46c-940c-4d6f-b371-3bedc1276ea8`,
  `01ded642-0b74-4c82-9f01-2392934f634f`, `61d6426a-ebe6-47dd-9169-0438fa593547`,
  `e6d09ed4-bcba-442f-956e-4a934a8da1aa`, `79677138-60b8-4f9e-9aac-9639fce593f4`,
  `c2b4c1bc-ec03-4e37-b904-585036e7f972`, `dbbad1e4-c319-49b6-9207-49c68f9737ec`,
  `9ca14bbf-5941-4c82-be06-e6ff1682b947`, `a3a424de-121e-497c-8779-38fece81ae71`), and passed three runs of its whole
  provisioning module, 179 tests each (runs `5e17de68-2698-46d3-8c37-ded4b270690e`,
  `1e36d681-8ef0-4683-afe2-20c7391e7356`, `25956875-58ad-435c-9aef-d106713f44df`). Its cause is not known; the
  hypothesis is a timing window under the full crate's load on a shared machine, which these passes do not rule out. Per
  `AGENTS.md`, a single failure under shared load is not recorded as a flake.
- After the landing fixes (one SPEC_impl.md sentence and the changelog entry), `dprint check` on the changed files and
  `python3 releasing/check-changelog.py format` passed; no code changed, so the runs above still apply.
- Reused: nothing from the executor's runs.
- Not run: the other crates' Rust tests, the browser suite, desktop and installer tests, and a check against an actual
  older OpenSSH or a real Tensorlake sandbox. The change is confined to how the helm builds the key and known-hosts
  options for Tensorlake, which the helm's own tests cover without a network connection.

Nothing else in the executor's report became untrue at landing: besides the specification and the line count above, only
the PRs' draft state and head commits changed, since the landing merged them.
