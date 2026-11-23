## What this was about

With automatic Claude Code folder trust on, starting a session in a parent of your home directory could grant Claude
trust for your entire home. Claude extends trust to everything inside a trusted directory, and those entries can persist
after the switch is off. An invalid home directory could also let Farhelm edit Claude's settings without being able to
check whether the launch directory covered your home.

The maintainer chose to leave Claude's own trust prompt in charge at home and above it, including on disposable hosts.
Farhelm now skips automatic trust for the home directory and every parent, including the filesystem root. It also skips
the edit when the launch shell supplies a home directory that is missing, relative or cannot be resolved. Farhelm
continues the launch and gives the reason in the terminal. Claude can then ask for trust. Settings and the Claude Code
and Start a session docs explain the directory rule.

## Things you should know

The guard applies both to directories you choose and to new managed checkouts, including disposable environments.
Previously recorded trust entries stay in Claude's settings. The change governs future trust edits.

## Open questions and possible follow-ups

None.

## PRs

- [#4664](https://github.com/scode/farhelm/pull/4664/changes) — Skip automatic Claude trust at and above home and
  explain the rule in Settings and the docs.

## Checks

- `cargo nextest run -p farhelm-supervisor --lib`, launch, Claude trust and retirement selection, passed all 107 tests
  on the draft (run `8ce6992e-df63-4189-86d6-eb9ff055d9c4`). The initial run failed one new fixture when it reused a
  launch file that Farhelm consumes on first use (run `d58f54e4-d585-462b-9b4a-9a9c3a5f0aab`). With a fresh file for
  each case, the exact regression passed (run `eaa19606-b1f6-4180-8ef1-4435ef841370`). Farhelm launched a test stand-in
  for Claude with four unusable home settings, explained each refusal and left the Claude settings file intact.
- `cargo nextest run -p farhelm-gui --lib`, Settings search and agent trust selection, passed 13 tests (run
  `7e5a5594-ce83-4704-827d-a3387c004021`).
- `python3 scripts/check-gui-goldens.py --update --scene settings-claude` showed two changed Linux references (run
  `b892e666-cde1-4cb0-9ebe-560d1a83ce93`). The full `--update` of 406 pictures found the same two changes (run
  `7e6b5871-6d56-4529-bce7-d19fcfbe492b`). I reviewed and published both themes.
- `cargo clippy --all-targets -- -D warnings` passed on the exact draft. `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` on changed Markdown and
  `python3 releasing/check-changelog.py format` passed. The website's `bun run build` built 39 pages and checked its
  links.
- Settings tests and the focused render covered snapshot `9f96bb2b`, the full render covered `b0b7203e`, and the website
  build covered `df44cbb8`. Their GUI and docs sources are identical in the draft apart from the two published
  screenshot references. The rebase picked up queue and TODO additions. Linux fixture tests cover the affected behavior,
  so broader runtime suites and live Claude runs were skipped. The Claude Settings reference set is captured on Linux,
  so a macOS capture was unnecessary.

## Review gate

The independent source review of the first candidate found one compile error in the new test and no production findings.
The fixture corrections were verified by the exact regression. The commit wording passed its separate cold read.

### Landing

#### What landed with it

The plan's one PR (#4664) landed alone in this round, followed by one landing fix (#4667). Nothing but queue and TODO
bookkeeping reached main between the plan's base and its landing, so the rebase was trivial. The plan narrows the "trust
in every folder" change of #4583, which shipped in v0.44.0 and v0.44.1; that is why it carries a release-notes draft of
its own rather than editing #4583's.

The landing's independent reviewer found no way left for a session folder that is the home directory, a folder above it
or the filesystem root to get a trust entry, whether it is reached through a symlink, `..` or a trailing slash, or the
home directory is itself a symlink. A home directory that is unset, empty, relative, missing or not a directory skips
the edit too, also when Claude's configuration folder is set separately. New managed checkouts go through the same check
as folders the user chooses. Removing trust when a managed checkout is archived or deleted is unaffected: it works from
the entry Farhelm recorded when it added that checkout's trust, and never runs the new check.

#### Fixes made while landing

The reviewer found all three; #4667 fixes them.

- When Farhelm skipped the trust edit on purpose, the session's terminal reported it through the line used for real
  failures, so a deliberate choice read as an error: "Farhelm could not mark this folder trusted. Claude may ask.",
  followed by "the session folder is your home directory or one of its parents, including /. Claude's trust covers
  everything inside", whose "including /." reads as if the folder were the root even for a session started in `/home`.
  The line now says "did not", which still fits a real failure (another program holding Claude's settings file locked,
  say), and the reason reads "the session folder is your home directory or a folder above it, and Claude's trust would
  cover everything inside it".
- The docs' table of agents still said Claude trust skips only the home and root directories, unlike the two docs pages
  the plan updated; it now names the folders above home too.
- The plan's release-notes draft gains two caveats: a trust entry v0.44.0 or v0.44.1 already wrote for such a folder
  stays in Claude's settings until the user removes it, and the edit is also skipped, with the reason in the terminal,
  when the home directory cannot be resolved.

#### Checks

The plan changes how the supervisor decides whether to mark a session's folder trusted for Claude, which every launch
with the switch on passes through, plus one Settings sentence with its two reference screenshots, the specs, two docs
pages and a release-notes draft. The helm and the desktop app's own process are untouched, so the landing ran the
supervisor's tests, the farhelm command's integration tests and the desktop app's interface tests rather than the whole
workspace, with the lints, every reference screenshot and the docs site build, on the plan as delivered; then the
supervisor's trust tests again after the landing fix. The worker runs were on a separate arm64 Linux test worker with a
systemd user manager, rootless Podman and SSH to itself; the rest ran locally on x86_64 Linux. None of them ran a real
Claude Code: the tests use a stand-in for it, as the report says.

On the plan as delivered, rebased onto main:

- `cargo nextest run -p farhelm-supervisor -p farhelm -p farhelm-gui`, on the test worker (recorder run
  `41dc41f0-94a5-42c9-b723-443b5af5c93d`): all 5690 passed, the real Podman environment test included. One of them, a
  test of the warning shown when stopping a container fails, printed a runtime `SKIPPED` because it found no usable
  systemd user manager that time, so that run is no evidence for it. Run again on its own,
  `cargo nextest run -p farhelm --test e2e -E 'test(failed_podman_stop_warns_on_restart_and_in_the_delete_reply)'` on
  the test worker (run `fa6fb95a-ac1b-42a5-837f-2ac8a86334a0`), it ran fully and passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `57d4023e-ef4d-41dd-93fd-1ddba79bc82d`): every picture
  matched its reference, the plan's two republished Claude Settings pictures included.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in `website/`), locally:
  passed.

After the landing fix (#4667):

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/trust/) | test(/plain_claude_launch/)'`, on the test worker
  (recorder run `76c23e96-3ab8-4493-98a3-31abf89b271c`): all 29 passed, among them the home check's test, which matches
  the reworded reason, and the launch test of unusable home directories, which reads the terminal line.
- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format` and the docs site build,
  locally: passed.

Reused: the suites above for everything else, since the fix changes only the wording of one terminal line and one
reason, a comment, one docs tooltip and the release-notes draft. Skipped: the helm's tests, the desktop app process's
tests and the end-to-end smoke test, which nothing in the plan reaches; macOS, where nothing compiles differently and
none of these tests depends on the platform (the macOS path in the notes below is about which paths the check
recognizes, which no test here covers on either platform); a real Claude Code; CI, which runs only on demand.

#### Things you should know that the report does not say

- The report says trust entries recorded earlier stay in Claude's settings. What that means for users of v0.44.0 and
  v0.44.1: a session started in `/home` or `/Users` with the switch on wrote an entry that makes Claude trust everything
  in the user's home directory, outside Farhelm too, and Farhelm never removes it. Farhelm removes only the trust it
  added for a new managed checkout, when that checkout is archived or deleted, never an entry in a folder the user
  chose. Removing such entries, or telling affected users, would be a follow-up.
- The check compares resolved paths, so a second path to the same directory that is not a symlink, such as a bind mount
  of `/home` elsewhere or the macOS path `/System/Volumes/Data/Users`, is not recognized as a folder above home. A
  session started there on purpose can still get an entry, which then covers that second path's tree only, since Claude
  matches trust by the path it sees.
- No test starts a session in a usable home directory with the switch on and checks that Claude's settings file is left
  alone. The plan's new launch test covers only unusable home directories, and the home check is tested directly. A
  queued review finding already asks for that test (the one on missing regression tests for Claude and Antigravity
  trust), but it still cites a helper and a test this plan removed, so its references need refreshing when someone takes
  it up.
