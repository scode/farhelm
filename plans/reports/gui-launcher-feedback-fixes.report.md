## What this was about

The maintainer selected four issues from the desktop-app triage. Two recent setups could look identical while launching
through different wrapper prefixes. Restart called some older Muse agent sessions command launches, even though those
sessions predated Muse's stored resume commands. The feedback dialog's Discord link did not show the arrow that signals
it opens a browser. The fourth issue was that a managed checkout in a new environment could get as far as Launch before
the helm reported the missing default checkout folder.

Recent setups now show the wrapper in the row, including for Bash and Zsh, and provide the full text on hover and
through assistive technology. Restart remains unavailable for older Muse sessions that lack a stored resume command. The
explanation now gives that reason and says that Replace starts a fresh session. The feedback dialog shows “Chat on
Discord ↗” and the link still opens the invite without closing the dialog or changing its draft.

## Things you should know

Long wrapper prefixes are shortened in the recent row. Hover help and the accessible name carry the complete prefix if
two rows still look alike.

## Open questions and possible follow-ups

Trying to launch a managed checkout in a new environment without an all-hosts default checkout folder still fails before
creating the environment. Farhelm's refusal points to Settings. Triage asked for the launcher to show the missing folder
setting at the point of launch and keep Launch unavailable until a save is confirmed. A selected host's own override
cannot satisfy that requirement. The fix needs to load the shared default and handle both a visible launcher and a
quick-launch shortcut. Triage allowed deferral when the fix needed broader work, so this is recorded as a Near term
TODO.

## PRs

- [#4406](https://github.com/scode/farhelm/pull/4406/changes) shows wrapper prefixes in recent setups.
- [#4408](https://github.com/scode/farhelm/pull/4408/changes) records the default checkout folder follow-up.
- [#4409](https://github.com/scode/farhelm/pull/4409/changes) fixes the Restart explanation for older Muse sessions.
- [#4410](https://github.com/scode/farhelm/pull/4410/changes) adds the feedback dialog's Discord arrow.

## Checks

- `cargo nextest run -p farhelm-gui --lib -E 'test(/^launcher::(recents_tests|recents_view_tests|search_view_tests)::/)'`
  passed 63 tests at working revision `76d4cdc4` (`0a5c2191-91c9-47f9-9a52-06af8d8e7532`). An earlier compile caught a
  missing test import (`259deef7-2ce8-4bdb-bfcc-1dd256486c4c`), which was fixed.
- `cargo nextest run -p farhelm-proto -p farhelm-supervisor --lib -E 'test(/no_resume_reason_follows_the_launch_kind|every_reporting_kind_says_when_restart_becomes_available|only_a_resume_offer_relaunches/)'`
  passed three tests (`db7dfe30-1634-492c-a48c-b2ee7ac1cef9`). The protocol and supervisor source in `9e6b7157` is the
  source this run covered.
- `cargo nextest run -p farhelm-gui --lib -E 'test(/^feedback::tests::/) | test(/restart_says_why_it_is_unavailable_and_sends_nothing|the_cards_words_say_what_ended_and_what_each_choice_does|the_hover_help_names_the_offer_then_says_why/) | test(/^launcher::(recents_tests|recents_view_tests|search_view_tests)::/)'`
  passed all 83 selected tests at working revision `f03469dc` (`433c2cd8-8dfc-41c2-8042-7f830a140f8a`), with no runtime
  skips.
- `python3 scripts/check-gui-goldens.py --update` captured 300 Linux reference pictures
  (`0f1cfdf5-1c95-458a-8cea-d66cc09c65f0`). Blob comparison found four changes, all in the two feedback dialog layouts
  in light and dark mode. I inspected all four and verified the published manifest matches the captures. The other 296
  matched their references byte for byte.
- Rust and Markdown formatting, changelog format, instruction references and Git whitespace checks passed. The
  instruction checker reported one existing archive citation warning. The test-delay check inspected 308 calls with zero
  missing rationales.
- The stack rebases kept concurrent changes to the triage and TODO records. Main also gained an updater change and tests
  for retired container removal and snapshot link grouping. The launcher, feedback and Restart code and tests covered by
  the runs are identical, so the results still apply.
- No runtime tests were needed for the TODO-only PR. The other workspace tests do not exercise the affected launcher,
  Restart or feedback behavior, which the selected tests cover directly.

## Review gate

Each draft passed a fresh gpt-6.2-sol high review and a commit-message cold read. Review led to a wrapper display that
reserves space, preserves its suffix when shortened, and shows the complete value on hover. It also made the
checkout-folder follow-up cover launch shortcuts and made the Restart tests verify the explanation for an older Muse
agent session and for a command launch using Muse.

### Landing

#### What landed with it

Nothing else landed in this round. Since the executor last rebased the plan, main had gained only plan bookkeeping (a
planning change that added plan references to other TODO entries landed during the round). An independent reviewer and I
read the four PRs. Each of the three fixes meets its triage outcome. Deferring the fourth item (setting up the default
checkout folder from a new-environment launch) to a Near term TODO entry is what its triage outcome said to do when the
fix needs broader work, and it does: the launcher has nothing that reads the all-hosts default today. The Restart
explanation change needs no protocol version bump: the supervisor sends the explanation to the app as plain text.

#### Fixes made while landing

One went in as a fifth PR, [#4420](https://github.com/scode/farhelm/pull/4420/changes). SPEC_impl.md and the
instructions agents read in their sessions still said that Restart reports a missing stored resume command only for a
command launch that did not opt into Resume. An agent seeing it on an older Muse session would have misreported why
Restart is unavailable. Both now also name an older agent launch from before resume commands were stored.

#### Checks

The plan changes the desktop app (recent setups in the launcher, the Restart explanations, the feedback dialog), the
shared protocol code that words Restart's explanation, the supervisor's record of each session, four of the GUI tests'
reference pictures (the feedback dialog), TODO.md and changelog fragments. The landing ran the desktop app's GUI test
suite, the protocol crate's tests and the supervisor's Restart tests, the app's end-to-end smoke test and the main lint
(clippy) on a separate arm64 Linux test machine, and the screenshot comparisons and small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `b255d952-26c1-4e2c-ab8f-87facf1ecb67`): all
  3226 tests passed.
- `cargo nextest run -p farhelm-proto -p farhelm-supervisor --lib` with the filter
  `-E 'package(farhelm-proto) | test(/restart|resume_offer/)'`, on the test machine (recorder run
  `222bd8fd-6999-41af-b253-cfe390f116a4`): all 335 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (recorder run `9d8429ac-a241-4fb1-8094-033549429dda`):
  every step passed but paste, which skips on Linux as it always does. Its steps include the feedback dialog and
  Restart.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (recorder run
  `8f963a90-d119-45b9-8448-d1b6e0ea4eb9`): passed, with the plan's four re-captured feedback dialog pictures.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

After the landing fix (#4420), which changes SPEC_impl.md and the instructions text agents read:

- `cargo nextest run -p farhelm --bin farhelm`, on the test machine (recorder run
  `c814e2fa-c80e-4812-9d2f-7c429ec1ba66`): all 137 passed, including the check that the instructions' lines stay narrow
  enough.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed. `dprint check` and
  `cargo fmt --all -- --check`, locally: passed.

Skipped: the workspace suite, since nothing the rest of it covers changed; and Mac runs, since nothing here is
Mac-specific.

#### Things you should know that the report does not say

- A session from before Farhelm recorded whether a session was an agent launch or a command launch, with no stored
  resume command, still gets the command-launch wording ("this command launch did not opt into Resume") in Restart's
  hover help and the supervisor's refusal. Those are a different, older group than the agent launches the fix covers.
  That predates this plan and was outside its triage outcome; a small follow-up could give such sessions their own
  wording.
- No test or screenshot shows a recent setup's row with its wrapper prefix drawn; the tests check the hover and
  accessible text behind it. The row allows the wrapper about 144 points, and its "· Wrapper: " label takes about half
  of that, so a longer prefix is cut short in the row (the full text stays in the hover help).

#### What you should still do

Nothing. If you want to see the wrapper row, a recent setup launched through a wrapper shows it in the launcher.
