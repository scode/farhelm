### What this was about

The send feedback window worked, but its bright fields and oversized labels did not match the app. Send and cancel
touched, the disabled send button faded, and there was no close button at the top. This plan implements the approved
mockup while keeping what is sent and how sending works unchanged.

### Things you should know

The dialog now has a feedback tile, privacy summary and × close at the top, dark fields, quieter labels and a recessed
display of the attached version, app and operating system. Cancel and send are separated in a pinned footer; only the
body scrolls in a short window. The footer says nothing is sent until send, or nothing was sent after a failure. The
hint disappears while sending and after success.

The × waits for an in-progress send exactly as cancel and Escape do, including a click queued immediately behind send.
Initial focus, contact reuse, failures keeping the draft, successful thanks and automatic close remain unchanged. Styles
are confined to feedback; app settings and host dialogs keep their existing presentation. The frozen v0.25.0
release-note screenshot was left as required.

### Open questions and possible follow-ups

None identified. The app settings window's separate planned layout work is outside this change.

### The PRs

- [Draft PR #1946](https://github.com/scode/farhelm/pull/1946/changes): finish the feedback dialog's styling.

### Checks run, reused and skipped

- Passed `cargo build`, `dx build --package farhelm-ui --platform web --release`, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo check -p farhelm-ui --features desktop`.
- Passed `python -B scripts/check-test-sleeps.py` using its isolated pinned parser environment: 295 delays inspected,
  zero unannotated. Passed `dprint check TODO.md`, `python3 releasing/check-changelog.py format`, and the diff
  whitespace check. dprint has no TypeScript formatter and excludes changelog fragments; Playwright parsed the tests,
  and the changelog checker inspected the fragment.
- Recorder run `ef9adc3c-fb62-4353-aeb7-99fc93296d44`: all 26 feedback and tooltip cases passed, 13 each on Chromium and
  WebKit, one worker and zero retries. The run covered the complete dialog change before the final two feedback-only
  body-font adjustments. Its menu, tooltip and contact-memory evidence still applies because those behaviors and
  controls did not change.
- Recorder run `d91e5dc6-0d10-4c5b-823e-4bf7375e50eb`: all eight selected final typography, geometry and send-state
  cases passed, four per engine, one worker and zero retries. Inspected screenshots of empty, filled, sending, failed
  and sent states, plus short and narrow windows, against the approved mockup.
- Recorder run `8a480966-3e96-45eb-97b9-bdfe62068eba`: both corrected short-window layout cases passed, one per engine,
  zero retries. The test now proves that the body actually scrolls before checking that its header and footer stayed
  pinned. Reused the prior send-state and tooltip evidence because only this test assertion and its explanatory comment
  changed.
- Reused the Clippy and desktop compilation results after the CSS-only typography refinement; Rust behavior and feature
  requirements were unchanged. The careful rebase crossed only another plan's claim, with no product or specification
  changes.
- Skipped the full Rust and browser suites: the change is scoped dialog markup and CSS, and the selected real-browser
  cases cover its lifecycle, payload, rendering and tooltip risks. No website or release pipeline changes needed those
  builds or tests. WebKit covers the desktop engine family; this was not a manual run of the Mac app.

### Review gate's outcome

The required single fresh-context gpt-6.1-sol review at high effort found no product-code issue and one gap in the
scrolling proof. That test could pass when fields were clipped rather than scrollable. Added assertions for
user-scrollable overflow and actual scroll-offset movement; the same reviewer rechecked the correction and found no
remaining issue. The commit/PR wording also passed its independent cold read.
