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
