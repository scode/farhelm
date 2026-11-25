# Late-refusal half of the trust-hint error fix is untested

Reviewed commit: 6ada98e54471a9b7213e4a8f3fad89903418f7dc

## TLDR

The fix for a stale error under the workspace trust hint has two parts, and only one is tested. If the second part
regressed, a user who dismissed the hint elsewhere while their own Don't show again was being refused would again see an
old save error when they turn the hint back on, and no test would fail.

## Details

The commit message says the error is cleared "including when an earlier save refusal arrives while the hint is
disabled". That case is handled only in `hide_settled` (`crates/gui/farhelm-gui/src/session_pane/trust_band.rs` at
6ada98e544, ~316-327): `failed = (refused && shown)` where `shown` comes from `settings::trust_hint_shown`. The updated
test `a_refused_dont_show_again_says_so_only_while_its_session_is_open` (`trust_band_tests.rs` ~666-730 at the tip)
covers a refusal followed by a pushed dismissal, which `follow()` clears, and a pushed dismissal followed by a 204. No
step refuses the save after `dismissed_is(Some(true))`. Without the `shown` guard, a refusal landing while dismissed
sets `failed`; if the next store change is the re-enable, `follow()` sees dismissed false and keeps it.

Fix: add a step that presses Don't show again, pushes `dismissed_is(Some(true))`, refuses the save, then pushes
`dismissed_is(Some(false))` and asserts `parts() == (true, false)`.

Suggested bucket: `other` (test coverage for a misleading-text fix). Confidence: confirmed from the test source; that
the regression would go unnoticed follows from reading `follow()` and `shows_failure`, not from running anything.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g01-hint-bands-hooks` record `f3`.
