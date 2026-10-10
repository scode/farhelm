### What this was about

This carries out the thirteen small UI bugs selected together during triage. Creating or replacing a session could be
followed by an older sidebar reply that closed the newly opened session. A failed attempt to mark notifications read
stopped later closes from retrying. An already-open host-update popup stopped following the active step, and an earlier
setup or uninstall error could hide the result of a later Update.

The session launcher could save an unfinished template name when Enter confirmed an input-method candidate. Non-ASCII
template names lost exact-match priority when their case differed, and the names `.` and `..` could be saved even though
requests treated them as URL navigation. Applying a Grok template after another agent retained an unsupported
workspace-trust choice. Terminal text-size buttons lost input focus at either size limit, and dragging to select a
program's hyperlink could open it. Rename and Restart with did not expose hidden characters or isolate text direction in
refusals; Rename also concealed such characters in its passive current-title display.

All thirteen fixes use the mechanisms already present in Farhelm. No outcome was dropped, deferred or found already
fixed, and none exceeded its individual complexity gate.

### Things you should know

Rename changes only how its current title and refusal are displayed. The editable draft and stored title retain their
raw characters. Template validation refuses exactly `.` and `..`; names such as `...` remain valid. Grok normalization
affects shared template application, including the CLI; the browser already normalized that choice separately.

The hyperlink selection check applies before both web opening and file downloading. File downloading already had its own
selection check, and the regression also verifies that an ordinary click still works. Starting Update clears earlier
setup/uninstall diagnostics while retaining the existing rule for uncertain update results. The browser checks exercise
the web UI under Chromium and WebKit; they cover the renderer family used by the desktop app, not the native desktop
shell. The IME regression dispatches a composing keyboard event; it does not drive a native input method.

### Open questions and possible follow-ups

None from this plan. Every selected outcome is included, with regression coverage using existing unit or browser
fixtures. No product or specification decision was needed.

### The PRs

[Draft PR #1832](https://github.com/scode/farhelm/pull/1832/changes) — all thirteen outcomes, verified open and draft at
head `2e172a8e9dbdedd737884474148ffbac805afb57`, based on main. The executor marked none ready and merged none.

### Checks run, reused and skipped

- Focused launcher and session-launcher units:
  `cargo nextest run -p farhelm-proto -p farhelm-ui --lib -E 'test(launcher::tests::) | test(launch_composer::tests::)'`,
  recorded run `e0da88b7-9c3d-449e-8c87-10b435ccc74e`: 61 passed, 524 excluded by selection, zero failures, four slots
  and zero retries. Pinned nextest and tmux were verified. The three new unit regressions passed. Reused because later
  Rust edits were comments and formatting, and subsequent fixes affected browser tests only.
- UI asset JavaScript harness: `cd crates/farhelm-ui/js-tests && node --test`, recorded run
  `9a6f163e-6086-4b99-b242-138cb2b352f7`: 230 passed, zero failures or skips, complete source and lifecycle identity.
  Reused after later edits limited to browser-test assertions, import organization and Markdown; the JavaScript asset
  remained unchanged.
- `cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo fmt --all -- --check`,
  `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md releasing/changelog.d/ui-correctness.md`, changelog
  format lint and the isolated test-delay checker passed. The delay checker inspected 282 delays with no missing
  rationale. Rust behavior was unchanged by the later browser-test correction. dprint has no JavaScript or TypeScript
  plugin here, so no formatter pass is claimed for those files.
- Browser regressions: thirteen scenarios in each engine, 26 passing cases across three successful recorded commands,
  all with one worker, zero retries, verified pinned tmux and complete source/lifecycle/output evidence. Pre-create
  listing protection passed 2/2 in `487dd448-5287-4d90-9fb7-5e1069418368`; reused after the unrelated Restart with test
  correction. The corrected Restart with refusal case passed 2/2 in `7a851e15-3fcb-4cfe-b8fa-a80b116b7f4b`. The other
  eleven scenarios passed 22/22 in `8f0bc242-49b3-41b4-a92a-d2c66ec91eb1`. Each engine ran every scenario, with no
  skipped or unstarted cases in these successful commands. Application and fixture builds were prerequisites, not
  runtime evidence.
- An initial Rust recorder invocation refused before executing tests because the validation container's Git ownership
  check hid the checkout identity. An earlier JavaScript run executed successfully with that identity unavailable. Both
  records were retained privately; only the distinct, corrected, identified observations above support the pass claims.
- Browser run `abbd56f0-d20a-47dc-b278-20b3aaaf6bb2` was deliberately interrupted after browser-cache discovery failed:
  21 launch failures and five unstarted cases, no product assertions or passes. The cache path was repaired only inside
  the owned validation sandbox. Run `3143cfe0-7ef9-43dd-ba6f-fff4acf837e6` then launched both engines but had one
  failure and one timeout before the stale-listing action because the test omitted its fake feed's greeting. The test
  now establishes feed health and retires greeting reads before holding the stale response. Both observations remain
  retained; later passes do not erase them. These were preparation and newly authored fixture defects, not classified
  latent flakes.
- Run `1646e33b-576f-4270-ac15-1deb65c28ea0` completed with 22 passing cases and two Restart with timeouts. Both timed
  out before requesting the refusal because the test tried to submit an unchanged draft. It now changes a launch choice
  and verifies the submit button is enabled. That failed command remains retained and is not claimed as a pass; final
  coverage uses separate successful commands.
- Broader Rust and browser suites, desktop runtime, installer and release checks were skipped because the selected
  regressions cover these bounded changes, while those gates target behavior outside this diff. The desktop renderer was
  compiled; native-shell behavior is not claimed as tested.

The identified unit and JavaScript runs captured working-tree edits on Git HEAD
`3f0e21b39e841dde0e1f0ca325ccb8bad70b674f`, source fingerprint
`126e3043c4cf121d27079bfbbc209e280264cf01885804539d5e0560d4641999`; HEAD alone is not their tested revision. Final
Restart with and other-browser runs captured the same HEAD with fingerprint
`19bd437238b7a158d2e6fcf0a65aa7336284928d6199e0f084ab85041ec33282`. The earlier pre-create proof captured fingerprint
`2e5dd34a0fc9f5f52456938a07d2879d67aa7f31316a64c2b5c2f8ad8a650fd8`; between the successful browser runs, only the
unrelated Restart with test changed. The JavaScript asset and Rust behavior remained unchanged by these browser-test
corrections and publication bookkeeping.

The PR was carefully rebased onto main `9b9dc8ce7500c9836f18664c3c50b7afc7b58baf`. The complete intervening diff changed
only another plan's queue state and delivered report. The PR's patch was byte-identical before and after rebase, so the
successful commands still cover the change; no additional runtime checks were needed. The official retained-run summary
was archived privately with complete discovery of ten observations and their preparation/identity/interruption gaps. Its
totals include repeated cases and are not the final 26-case coverage denominator.

### The review gate's outcome

The required fresh gpt-6.1-sol/high source review found one weakness in the IME regression: its later intended save
could be confused with a premature asynchronous save. The test now edits an unfinished name to a distinct final name and
verifies only the final name is stored. That follow-up and the later listing and Restart with fixture reviews passed
with no remaining findings and no production defect or complexity-gate violation. The full test-authoring contract was
supplied verbatim. A separate documentation pass inspected every touched file. Static review did not catch the missing
greeting or changed-draft premise; runtime validation exposed them. The corrections preserve the withheld old and fresh
listing responses and exercise the ordinary restart refusal after an admitted edit.

Commit and PR wording received fresh cold reads. Actual native model attribution and usage were not exposed by the
harness; requested routes are recorded separately from that evidence gap in the private galaxy-brain session records.
