# Antigravity workspace trust

## What this was about

When Antigravity stopped to ask whether to trust a directory, Farhelm could show it as idle and gave you no way to skip
the question on later launches. Farhelm now shows the session as Waiting while the question is on screen. In a session
started as an Antigravity agent, a note above the terminal links to **Settings → Agent trust** when the new switch is
off. You can hide the note for this occurrence or dismiss Antigravity's note across sessions and restore it in Settings.

## Things you should know

The switch is off by default. Turning it on trusts the session's working directory when you start a new Antigravity
session or use Clone, Restart with or Replace with. This covers directories you choose, managed checkouts and disposable
environments. This lets repository hooks, settings and commands run with workspace trust. Farhelm passes that directory
to Antigravity for this process in place of its saved trust list. The saved settings file stays as it is. Custom
permission flags supplying `--settings` take precedence. Plain Restart and Replace reuse the session's saved launch.
Command launches use the arguments you supply.

Protocol 82 is a release-testing point. The helm and remote supervisors need matching builds. Include a host update when
testing the release across machines.

## Open questions and possible follow-ups

Antigravity's Custom permission flags can end with a bare `--`. Farhelm puts the new trust option before that separator.
The existing launch layout can still put an initial prompt or conversation-ID option after it. In that case, the CLI can
miss the selected conversation or interpret the initial message differently from what you entered.

## Pull requests

- Detect the real Antigravity trust menu and report Waiting. [#4349](https://github.com/scode/farhelm/pull/4349/changes)
- Apply a per-launch trust opt-in and preserve Custom settings.
  [#4350](https://github.com/scode/farhelm/pull/4350/changes)
- Add the switch and discovery hint to the app and document their scope.
  [#4356](https://github.com/scode/farhelm/pull/4356/changes)

## Checks

- Ran Antigravity CLI 1.5.13 in disposable directories on Linux and captured the trust question in wide and narrow
  terminals without answering it. The sanitized fixture preserves the captured screen's blank rows below the footer,
  which Git's whitespace check reports. Filtered
  `cargo nextest run --locked -p farhelm-proto -p farhelm-supervisor --lib` passed 53 screen-reader, trust argument and
  protocol cases on `c1fb1f20` (recorder `4b3d56b1-ca0f-4c7f-974b-16b1153ee983`). Only the commit message changed before
  #4349 was pushed.
- `cargo nextest run --locked -p farhelm-helm -p farhelm-proto -p farhelm-supervisor --lib`, covering all Helm library
  tests and the related protocol and supervisor cases, passed 1,780 and failed ten on `677a3053` (recorder
  `ca027ac9-381a-425d-bd7e-fed0f6c4b25c`). Eight failures came from tests that rewind the database to an older schema
  but left the new preference column in place. Two provisioning tests used a stale protocol-81 development binary. After
  correcting those fixtures and rebuilding the binary, a focused run including all ten failures passed 33 tests on
  `af003e90` (recorder `28e24a93-2627-48f4-a039-9b120152d5bf`). Only the commit message changed before #4350 was pushed.
- Filtered `cargo nextest run --locked -p farhelm-gui --lib` selected settings, trust hints and approvals and passed 350
  of 351 on `35d9f136` (recorder `121eccfa-3ae0-4da4-831d-67385c149f54`). One test's sample JSON omitted Antigravity's
  dismissal field. With the fixture corrected, the focused recheck passed 53 on `90cd3135` (recorder
  `1f30c7c7-7e95-48b1-96fc-6c0f6b7db3d5`), including the hint in an ordinary directory and focus on its switch. The
  source was unchanged when the golden manifest and commit message were added.
- `python3 scripts/check-gui-goldens.py --update` rendered 300 Linux captures on `90cd3135` (recorder
  `5dde2520-7b0e-43c7-87b5-d10cfc4e7a3f`). The existing 298 matched their references. I inspected the two new light and
  dark views, published them, and verified every capture against the fetched references.
  `python3 -B scripts/test-gui-screenshot-batch.py` passed 47 tests with the updated manifest (recorder
  `22581ae0-c4f4-47d1-877c-5db0e33a3a10`).
- `cargo clippy --all-targets -- -D warnings` passed for protocol, supervisor, helm and GUI. Rust and Markdown
  formatting, test-sleep, changelog, and docs build and link checks passed. Main advanced through plan queue and report
  changes, which do not affect the tested code. The focused Linux selections exercise the changed launch,
  screen-reading, storage and UI behavior. Full workspace and macOS coverage remain for landing and release checks.

## Review

All three PRs passed their source and wording review gates. The reviews tightened trust-menu recognition and kept older
command arguments literal, added coverage for competing Custom settings, and corrected the docs and screenshot framing.
The existing Custom separator limitation above was left for a separate change.
