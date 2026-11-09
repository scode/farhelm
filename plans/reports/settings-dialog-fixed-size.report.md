# Settings keeps one size across sections

## Since the last review

PR #3231 remains the only PR; none was added or dropped. Its feature patch is unchanged after rebasing. It was blocked
because Notifications could not fit without scrolling. The recorded decision now accepts its scrolling inside the 780px
frame and keeps the form as it is. The existing implementation already does that; this round reconciled the draft and
its evidence with that decision.

## What this was about

Switching Settings sections changed the dialog's height and moved its Close button. Settings now keeps a 780px height,
capped by the window, on desktop and phone. Short sections leave blank space; long sections scroll inside the fixed
header and footer. The section list on phones shares that frame too.

## Things you should know

The height was increased from the initial roughly 640px proposal because the ordinary desktop Terminal controls and
Checkouts did not fit. Source-derived browser measurements put the native Terminal's minimum near 727px; the actual
application needed about 774px for Checkouts. The chosen 780px accommodates both. Agent trust, Notifications and long
recipe or credential lists still scroll. Smaller windows cap the frame and can require other sections to scroll too.

The native Terminal measurement used its source markup, stylesheet and fonts in browser engines. It did not run the
native desktop app or macOS WebKit. Chromium and WebKit exercised the real application's fixed frame, every section,
phone list/back path and scrolling Agent trust. The width and existing body scroll container are retained.

## Open questions and possible follow-ups

No decision remains open. Native desktop appearance was not exercised directly; the native Terminal sizing evidence has
the limitation above.

## The PRs

- [#3231 — Settings stays in place when sections change](https://github.com/scode/farhelm/pull/3231/changes).

## Checks run, reused and skipped

- Run this round: changed Markdown formatting, changelog-fragment format (19 fragments) and patch whitespace checks
  passed. The careful rebase onto `6a329a7c` preserved the feature patch exactly. Intervening changes concern remote Mac
  provisioning, its documentation and queue bookkeeping; none changes Settings layout, its content, fonts or fixtures.
  The subsequently landed compiler pin and launcher effort-label correction also leave those inputs unchanged.
- Reused the final 780px application run `b3ead4c5-ecac-4012-9f61-b93492fcf776`:
  `npx playwright test '/settings\.spec\.ts$' -g 'settings keeps both close controls|settings navigation keeps'`,
  through the recorder with pinned tmux 3.7c and zero retries. All four selected cases passed, two per engine; no skips,
  flaky outcomes or timeouts. The retained output is complete. Its tested source fingerprint is
  `49097c0e62bc4613d02311abc3064ea5b6e3407b61562c30db1db71fe330bbe1`, and its recorded working diff matches the pushed
  final draft exactly. The unchanged patch and unrelated upstream changes preserve this command's coverage.
- Reused source-derived native Terminal sizing run `7c44be84-05e6-42bf-9dd1-b060430b01eb`,
  `deno run -A <private-sizing-script>`, on revision `91a22da9` plus recorded source fingerprint
  `efa67d845a8abcef3ced79511cae4087d7fd25db78e37dbc3c304f680c11c66d`. Both engines fit at 760px, so the unchanged
  content has more room at 780px. The relevant source hashes still match; the changed fixed height only adds body space.
  The fixture's native-app limitation is stated above.
- Ran `python -B scripts/check-test-sleeps.py` with the isolated pinned interpreter on rebased draft `bce76be1` this
  round: 430 delays inspected, no missing rationale. No instruction or heading changes require an instruction-link
  check.
- Earlier 640px and 760px application attempts failed the Checkouts fit assertion. Those retained failures motivated the
  final height; they are not passing validation or evidence of a latent flake. The final four-case run covers the
  corrected behavior. A private sizing fixture's initial font timeout and an interrupted intermediate web build were
  also retained and are not counted as passes.
- Skipped new runtime runs, Rust, the full browser suite, desktop smoke, installer and provisioning checks: the product
  change is confined to the Settings frame and its layout tests; no additional uncovered interaction currently justifies
  those suites. No application runtime or native-execution coverage is claimed for the static checks.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design or idiomatic-code issues. It
checked the existing patch and retained application evidence against the accepted Notifications decision. Earlier
review's native Terminal sizing finding was addressed; the remaining acceptance issue is resolved by that decision.
