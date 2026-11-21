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

### Landing

#### What landed with it

Two other plans landed right after this one in the same round: model-family-cleanup (launcher model family shortcuts and
a shorter wrong-agent refusal) and template-placeholder-validation (launch templates refuse misplaced command
placeholders when saved). Since this plan's work started, main had gained only the Add host takeover wording fix and two
TODO entries. An independent reviewer and I read the three plans' changes against each other, and none of it interacts
with this plan:

- The other two change different parts of the same launch code.
- Model-family-cleanup stopped sending a separate shortcut word with each model in the launcher's list. That list goes
  only from the helm to the app built with it, so it needs no protocol change; this plan's move to protocol 82 is the
  round's only one.
- To trust a directory, this plan has the supervisor fill the session's working directory into Antigravity's trust
  option, through a placeholder only an Antigravity launch fills. A saved template carries no trust setting, and the new
  template save check treats that placeholder as ordinary text, as Command launches do.

#### Fixes made while landing

One went in as a fourth PR, [#4362](https://github.com/scode/farhelm/pull/4362/changes). None of it changes what you see
or what a launch does:

- In SPEC.md's Status section, this plan had dropped the words recording that OMP's and Grok's approval prompts showing
  only running or idle is a settled decision, not a missing feature. They are back, now naming Antigravity's approval
  prompts too, which this plan did not change.
- SPEC_impl.md now says that the working-directory placeholder above stays literal in a Command launch. A session's
  displayed command shows it as it is, so pasting that command into a Command launch does not carry the trust.
- The Settings help for Antigravity's switch now follows the settings code's rule of deciding each such text for every
  agent; the text is unchanged.
- The changelog fragment names the section "Agent trust", as Settings spells it.

#### Checks

This round landed three plans together: antigravity-workspace-trust, model-family-cleanup and
template-placeholder-validation. Between them they change the protocol (to 82), the helm's database (to version 93), how
the supervisor reads Antigravity's screen, the helm's launches, templates and preferences, the desktop app (Settings,
the note above the terminal, the launcher's model field and search, the template editor), screenshots, the docs website,
SPEC.md and SPEC_impl.md. So the landing ran the whole workspace suite, the desktop app's tests, the app's end-to-end
smoke test and the main lint (clippy) on a separate arm64 Linux test machine, and the screenshot comparisons, the
website build and the small checks locally on x86_64. All of these ran on the three plans stacked together on main.

On the plans as delivered:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`8bcf674c-ac40-4153-932e-8c1088275db5`): all 8182 tests passed, and no test reported skipping itself because the
  machine lacked something it needs (a systemd user session, ssh to itself, Podman).
- `cargo nextest run -p farhelm-desktop`, on the test machine (`6780e911-a7be-4097-ba35-431188b6f40b`): all 180 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (`084e7104-a303-472d-8b1b-04d8906d56b6`): every step passed
  but paste, which skips on Linux as it always does.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (`2980a7fb-c943-402e-99cf-5ac085118d90`):
  passed, with Antigravity's two new Settings pictures.
- `python3 -B scripts/test-gui-screenshot-batch.py` (which validates the screenshot list), the website build
  (`bun run build`), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

After the landing fixes, #4362 for Antigravity (one function in the app's Settings code, which still shows the same help
text, plus SPEC.md, SPEC_impl.md and a changelog fragment) and #4363 for model-family-cleanup (a docs page):

- `cargo nextest run -p farhelm-gui`, on the test machine (`c609aff2-d2f1-4346-9cb8-16f37e4a1266`): all 3226 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed.
- The website build, `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`,
  locally: passed.

Reused for the fixes: the workspace suite, the desktop tests, the smoke test and the screenshot comparisons, since the
Settings help shows the same text and nothing else that runs changed. Skipped: Mac runs, since no macOS-specific code
changed; the Antigravity note's devbuild check covers the Mac by hand.

#### Things you should know that the report does not say

The report's open question still stands, and it is wider than the report puts it. Antigravity Custom permission flags
that end with a bare `--` put all of the launch's later options after that separator: a conversation to resume,
Farhelm's own options and an initial prompt. Antigravity reads each of them as plain text. That was true before this
plan; the plan only makes sure its own trust option goes before the separator.

#### What you should still do

Try it on a devbuild with Antigravity installed: start an Antigravity session in a directory it has not trusted, check
that the session shows Waiting while the trust question is up and that the note above the terminal links to the switch.
Then turn the switch on in **Settings → Agent trust**, start another session there, and check that the question does not
appear. The protocol moved to 82, so update remote hosts when you test across machines.
