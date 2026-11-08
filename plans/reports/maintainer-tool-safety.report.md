# Maintainer tool safety

## What this was about

The terminal-driving tool could recursively delete unrelated work when `stop` received a directory that was not a
capture run. An empty argument could name the checkout itself. Separately, the screen-fixture scrubber and its
independent Rust privacy check could miss a local proxy URL containing a username or password before its host.

The maintainer chose two bounded fixes: leave unrecognized directories alone, and keep the scrubber's documented
replace-and-refuse promise. The stack makes `stop` fail without deleting a non-run directory, and recognizes userinfo
before localhost and bracketed IPv6 hosts in both URL guards. Each outcome's execution is recorded, and its feedback
file and index entry are removed in its own PR.

## Things you should know

The stop self-test covers an absolute non-run path, `.` and an empty argument, checking a canary's preserved bytes.
Normal start, input and real terminal-host shutdown checks still pass. The URL tests cover username-only and
username/password forms, with path, query-only and fragment-only suffixes. Public links with `@` in a path or query
remain allowed. The changes contain 24 and 76 added-plus-removed lines of code, tests and documentation, respectively,
within each 100-line gate; the agreed ledger, queue-removal and changelog exclusions apply.

No credential leak was observed. No vendor capture turns were spent, and captured screen files were not edited.

## Open questions and possible follow-ups

None required for this plan. The documented manual review of captured fixture diffs remains a backstop for personal data
outside the automated checks' patterns.

## The PRs

- [#3169](https://github.com/scode/farhelm/pull/3169/changes): refuse non-run stop paths and preserve their contents;
  draft head `19ad8b72`.
- [#3172](https://github.com/scode/farhelm/pull/3172/changes): scrub and refuse local fixture URLs with userinfo, with
  Python and Rust coverage; draft head `1d947057`, based on #3169.

## Checks run, reused and skipped

- Passed: recorder-backed `python3 -B scripts/agent-tui.py self-test`, run `e2d3aafe-4c6c-4c9c-9c48-bafe029b6b72`; the
  existing checkout-built Farhelm supplied the real terminal host.
- Passed: recorder-backed `python3 -B scripts/test-capture-agent-screens.py`, final run
  `ba86f5d6-43aa-4688-8561-fb83e39ffc82`, two tests covering 36 local URL cases and two public controls. Earlier run
  `0cf0f89d-31e6-445d-849e-f24efbd7b4e4` passed before query/fragment cases were added.
- Passed: recorder-backed `cargo nextest run -p farhelm-supervisor --lib` with filter
  `test(/^agent_kind::screen_fixtures::/) | test(/^agent_kind::status_line_fixtures::/)`, run
  `071fdbe4-480f-40e1-8cba-5005edbefe3a`: 10 passed, 1,784 selected out, pinned nextest 0.9.148, four slots, zero
  retries and `--tmux none`. Source capture, runner report and output are complete, with no runtime `SKIPPED` marker or
  cleanup limitation. The first attempt, `cb455126-f9c2-4344-8d19-addd3d4e51f9`, hit a five-minute command deadline
  during compilation, before any test ran; its retained evidence is incomplete as a test result. The final attempt
  allowed compilation to finish and kept the runner's per-test limits. This was no observed test assertion failure.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, isolated
  `python -B scripts/check-test-sleeps.py` (423 delays, none missing a rationale), changed-Markdown `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py` (zero errors, three existing
  citation warnings). An initial dprint wrapping error was corrected and the check passed.
- Reused: the clippy pass covers the final Rust logic; only two additional URL literals in its test changed afterwards.
  The stop self-test covered base `5b7172ca` plus its working diff; the final Python and Rust tests covered base
  `19ad8b72` plus the final URL working diff. Subsequent commit descriptions and ledger PR URLs do not affect those
  commands. Fresh main `f164d321` adds Credentials UI wording, its browser tests and specification, and plan/TODO
  bookkeeping; all upstream diffs were read and no interaction found. No rebase or runtime repetition was needed for
  those independent changes.
- Skipped: other Rust suites, browser, desktop, installer and provisioning tests. These maintainer-tool changes have
  focused script and fixture coverage; those suites add no identified coverage. No live installation or cloud check was
  needed.

## Review gate's outcome

Each PR received the required single fresh-context gpt-6.1-sol review at high effort. PR1 had no findings. PR2's
query-only and fragment-only URL gap was corrected, tested and accepted by the same reviewer. Separate cold readers
checked the commit and PR text. No finding remains unresolved.
