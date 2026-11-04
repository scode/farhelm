# Launch progress

## What this was about

A slow host left the session launcher looking stuck. After the app's one-minute request deadline, it could say “Not
launched.” even though the helm was still completing the launch. The same silence affected Restart and Replace.

The launcher now shows the host and elapsed seconds after two seconds, beside its disabled buttons. Quick launches and
template shortcuts show the same status in a bottom pill. Plain Restart and all three plain Replace surfaces show the
wait too. Fast replies look as before, and the status disappears when the request settles. Assistive technology gets one
announcement rather than an announcement every second.

## Things you should know

The agreed scope stays in the GUI. There are no new server messages, launch steps, retries or cancellation controls. The
existing one-minute deadline and retry identities are unchanged. The displayed wait begins when the prepared launch
enters the API call; it can include waiting for preceding configuration saves.

Deadline expiry now says “Still waiting on [host]” and explains that the operation may still finish. New and Clone
retain the same-settings guarantee against a second copy. Replace makes that promise only while its original retry
identity remains: cancelling, changing settings or starting another Replace ends the guarantee. Restart and Restart with
ask the user to check the session before restarting again, because neither has a retry key. Actual helm refusals retain
their old wording.

Two implementation constraints needed adjustment. Browser engines discard the transport's typed timeout evidence
differently, including while reading a successful response's body. A request-owned browser marker now records the
existing deadline firing; it does not abort, retry or settle the request. The open session's header cannot fit a status
inside its fixed-width controls at a 650-pixel pane, so its status appears immediately below that toolbar. Before two
seconds it adds no height. The interrupted-session card and sidebar keep their status beside their actions.

The changed documentation page is `website/src/content/docs/docs/using/start-a-session.mdx`. Creation, Restart and
Replace behavior is recorded in the specs, the launch-feedback TODO entry is removed, and one changelog fragment covers
both PRs.

## Open questions and possible follow-ups

A dropped connection that is not a deadline expiry still uses the old “Not launched.” wording. That known limitation was
expressly outside this plan; it can still misdescribe an unknown outcome.

An additional host-terminal regression run stopped before any test because the harness checked for its startup session
while that session was still being created. The failed run is retained as `1287cb5b-7bcd-4ba4-9628-87100731c260`. A
narrow startup observation passed afterwards; this does not establish the failure's frequency or eliminate the harness
readiness risk. No product behavior was changed to work around it.

## The PRs

- [#2970: pending session launches](https://github.com/scode/farhelm/pull/2970/changes), draft,
  `c3387315057902f7798305da4fe2f4d3bce405a9`, based on main.
- [#2974: pending Restart and Replace](https://github.com/scode/farhelm/pull/2974/changes), draft,
  `bc9625b7a8cc75c0a16d4876c38f50ee45f767eb`, based on #2970.

## Checks run, reused and skipped

The final executable code passed the UI unit suite through the recorder: 842 tests, run
`3b6eccaa-7351-44cf-80e9-dd35020d6499`, using `cargo nextest run -p farhelm-ui`. UI test compilation,
`cargo check -p farhelm-ui --features desktop`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and the
release web build passed. Later edits only clarified comments and specification wording; they change no executable code,
doctests or generated artifacts, so those builds and tests remain applicable to the published heads.

These checks ran before the final changes were committed. Their working tree was based on
`53cdd2bad1e4a4d5b62c2bdc7dfe1cda34b9797d`, with the corrected deadline marker and second PR already present as local
changes; that old commit alone is not the tested source. The recorder identifies the unit-test tree by fingerprint
`d9cacc5ac21beed5468c1c57e3812028f8526fb818738bc55b4289e5304edd9c`, the full new browser run by
`48800614f2d746f643db86974f86770c931637bc6a99085a4bed59f7319f59ec`, and the host-new/default-existing runs by
`0d7cccb5b496c9a0b539e5785cc78ba14e2bcfb6d29d99ae5abba524807a9d87`. Only comments and specification wording differ
between these captured trees and the published executable code. The final host-existing run used the clean published tip
`bc9625b7a8cc75c0a16d4876c38f50ee45f767eb`.

Chromium and WebKit each passed all fourteen new browser cases in `launch-progress.spec.ts` and
`restart-replace-progress.spec.ts`: 28 executions in `5b9a33cf-751d-44ac-9876-527c1135fd1d`. The host-terminal variant
passed the same 28 executions in `62896857-45ff-41ff-971c-70ad8ba44481`. These cover the threshold and counter, host
name, quick-launch and template pills, accessibility, actual deadline expiry before headers and during a held successful
body, a timeout-worded server refusal, all Replace surfaces, retained retry identity, Restart with, and header geometry.
The two deadline cases also passed separately on both engines in `aa19d8b4-1ddf-48f2-9015-af3c874cd5b2`.

Six existing create/retry, disabled-input, refusal and Restart/Replace cases passed on both engines: twelve executions
in `2841077e-b284-4d1f-9268-c84f48e8e189`. The startup-only failure above ran none of those host-variant cases; the
narrow Chromium retry observation passed one case in `7b178b6e-d28b-4176-9d22-75306c368c44`, then all twelve
host-variant executions passed in `71c9ad2b-fff3-4fe8-901d-0b0de1b21484`. Successful recorded runs were checked for
complete reports, retained output through EOF, runtime skips and cleanup limitations.

Development failures are retained separately: `b79c7ea8-16b5-4a68-a960-0dd9328a1f58` reproduced the old response-body
timeout bug; `4f39d81c-c91e-4aa4-91bf-537e0d3907c1` and diagnostic `25de0895-afb5-474d-9ea2-ec7b1ec36f5a` exposed
browser error-shape differences and a test-helper capitalization error. The final marker and corrected tests passed
afterwards. These same-session failures are not counted as passes or latent flakes.

`cargo fmt --all -- --check`, Markdown formatting on the changed pages/specs, changelog format validation, and the
isolated test-sleep checker passed (397 delays inspected, none missing a rationale). The phone footer and 650-pixel
header screenshots were inspected. WebKit and the native-tunnel CSS preview supply engine and geometry evidence, not a
macOS native-app result.

The full backend, installer, provisioning, desktop runtime and browser batteries were skipped: no backend, protocol,
installation or native lifecycle behavior changed, and the targeted checks cover the affected GUI paths. No new control
needs tooltip coverage. The docs edit changes ordinary prose only, without links, headings or executable examples, so
formatting was sufficient and no website build or preview was started.

Main advanced to `64ff9b8603e4af685eb58510dcc8bd16ac81fafa` during execution. A separate read-only audit examined every
upstream product change: terminal recovery and shutdown checkpointing preserve accepted-request ownership, retry
identities and the GUI's deadline. Their specification and TODO edits affect separate passages. No functional
interaction required rebasing this stack or rerunning its checks before delivery. Landing still needs to integrate both
sides with its then-current main.

## Review gate's outcome

Each PR received the required fresh-context adversarial review by gpt-6.1-sol at high effort. All findings were accepted
and fixed, including hidden-pill accessibility, timeout evidence through response-body reads, Replace's retry-identity
ownership and header width. The final PR1 review passed after the corrected artifact's deadline tests passed on both
engines; PR2's follow-up found no remaining actionable issue. Fresh own-model scope checks approved the deadline marker
and forced header layout adaptation. Commit and PR wording passed their independent cold reads. Nothing has been marked
ready or merged by this executor.
