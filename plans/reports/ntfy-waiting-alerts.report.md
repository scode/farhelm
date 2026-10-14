## What this was about

Waiting sounds only reach someone near an open Farhelm window. This plan lets you configure an ntfy server and topic so
a session can alert you when it needs you. The agreed trigger is a session becoming waiting; Farhelm approval cards and
finished turns do not send these alerts.

The settings dialog now has an explicitly saved **ntfy alerts** section: enable switch, server URL, topic, optional
access token and priority, plus **send test**. The helm sends alerts, so a standalone helm can notify with no browser
window open. The stored token never comes back to the form; you can keep, replace or clear it.

## Things you should know

- Alerts start off. Save a server/topic, subscribe in ntfy, test the saved choices, then enable and save. Send test
  deliberately works while disabled and ignores unsaved form edits.
- Keep the desktop window open; closing it stops its embedded helm. Background windows still send alerts. Waiting
  detection remains heuristic, with the existing agent limits: OMP and Grok do not report waiting.
- Alerts contain the session's actual title and host display name. Controls are stripped and each field is capped at 200
  characters; a title derived from a directory is still sent. Anyone who knows an ntfy.sh topic can read it. The
  optional token is stored in helm.db without encryption; HTTP servers carry it and alert contents without TLS.
- Waiting settles for 10 seconds and sends once per spell. Startup/reconnect establishes a quiet baseline. Background
  sends are capped at five attempts per minute across the fleet; failures, excess alerts and a full 32-item queue are
  dropped, with no retries or later summary. A subsequent permitted alert can mention the rate-cap dropped count.
- The helm schema advances to 47. There is no supervisor protocol change. A careful rebase preserved unrelated
  release-test and planning changes and new TODO entries; none changed this feature's runtime dependencies.

## Open questions and possible follow-ups

No unresolved product decision blocks this delivery. A docs screenshot refresh after landing can illustrate the new
settings section; the publishing procedure accepts only captures of main's application. The short setup guidance is in
the session-list page. The website has not been deployed.

Browser checks cover Chromium and WebKit, including narrow layout and the native priority selector. They do not prove
the native Mac shell or receipt on a real phone. A release carrying this feature can be checked with an actual private
subscription and a background desktop window; no live installation was touched here.

## The PRs

- [#1986 — ntfy waiting alerts](https://github.com/scode/farhelm/pull/1986/changes), draft, head
  `56a7e9d27bd48f24ddc3e29dda558b3d2a003f5f`. One coherent change ships the sender with its settings and removes the
  covered TODO entry. No PR was marked ready or merged by this executor.

## Checks run, reused and skipped

Focused recorded Rust checks passed all 12 initial policy/sender/route/manager cases (`cf3884f8`), two UI/schema cases
(`88366c5d`), three corrected migration/manager cases (`643a5c5b`), and the final strengthened manager proof
(`4b1f36ba`). The required helm-package run (`a7d2beae`) had 1,068 passes and three failures: two historical migration
fixtures retained the new table, and the new manager fixture advanced time with a pending Waiting reply. Those fixtures
were corrected and the exact failures passed on focused retry; the other passing cases were reused because runtime
behavior remained unchanged. That package command itself is retained as a failed attempt, not relabeled a pass.

Browser settings coverage had 20 unaffected passes in the first attempt (`71834da7`). Its six failures across the two
engines were a priority locator and obsolete dialog counts/height assumptions. Corrected form/pinned cases passed four
(`89c3f22e`), final isolation/focus passed two (`d6621436`), and the priority dark-scheme proof passed two (`1c6ce184`).
The intervening two missed-count failures in `89c3f22e` are retained. Every selected behavior passed after correction;
there were no runtime skips. Earlier unaffected passes remain applicable to the final narrow CSS/count corrections.

The asset JavaScript harness passed 240 cases (`e24aa578`). All-target Clippy, desktop renderer compile, binary and web
builds, Rust formatting, diff whitespace, changed-Markdown formatting, changelog lint, and the isolated sleep checker
(298 delays, zero missing reasons) passed. The website build passed after the final setup wording, including internal
links. Test-build failures `ace4e949` and `63597e66` and all runtime failures are retained privately in the plan's
working log; later passes do not replace them.

No full-workspace, installer, provisioning, release, native-Mac runtime or whole-browser battery was added: the targeted
checks and required package run cover the changed behavior. Runtime checks were reused after the clean rebase because
landed changes touched release-test procedures, planning files and TODO entries, not application code or the specs this
feature depends on.

## Review gate outcome

The required single gpt-6.1-sol/high source reviewer found three issues, all corrected: browser tests needed an owned
loopback receiver, the manager test needed a distinct final title to prove causality, and setup guidance needed the
enable-and-save instruction. Its follow-up caught one remaining old checkbox count; corrected. The integrated review
reported no actionable findings. A fresh scope reassessment found no unnecessary substantial production complexity and
recommended the current design. Tests were reviewed with the full repository test-authoring contract. Commit and PR
wording passed a separate fresh cold read. These source reviews ran no tests; the execution evidence is above.

### Landing

Landed alone in its round as #1986. Between the stack's base and the merge, main gained only documentation and queue
bookkeeping commits, none of which touched what this feature depends on.

#### Release-relevant

The helm's database moves from schema 46 to 47 (the alert settings table). As with every schema bump, a helm that has
run this build cannot be downgraded in place. The helm–supervisor protocol version is unchanged, so remote hosts keep
working without an update. The helm now makes outbound HTTP requests to a server the user configures, only after they
enable alerts or press send test.

#### Fixes made while landing

An independent fresh-context review, focused on security, found nothing blocking. The landing fixed three points before
merging:

- Alert text was filtered for control characters only. Alerts now drop every character the repository's shared list of
  unsafe display characters names, which also covers invisible characters and bidirectional overrides. A session's title
  is set by its agent, and an override in it could make the alert on the phone read as if it named a different host.
  SPEC.md and SPEC_impl.md now say so, and the existing unit test covers an override, an invisible character and an
  emoji that must survive.
- When the helm cannot build its HTTP client (a machine without a CA bundle, say), it now logs why, once. Before, every
  alert only reported the generic "cannot make outbound connections" reason.
- The schema 47 migration step now carries the same error context as its neighbours.

#### Review notes left as they are

- Possible follow-up: the quiet baseline after a reconnect may not cover sessions whose first reading after a supervisor
  restart is "unknown". If a session was already waiting and its first fresh reading after the restart shows it still
  waiting, one alert can go out for it, within the five-per-minute cap. Treating "unknown" at the baseline as already
  handled would instead risk missing a genuinely new waiting spell right after a reconnect. Which way to lean is a
  product choice, so it was left as built.
- For a host without an alias, the display name in an alert is its ssh destination, which can include a user name. The
  spec already says host display names are sent.
- Two test nits were not changed. A refused settings save is checked only through the enable switch, and the
  stalled-sender test does not check which send its second request came from. The second runs under paused time, which
  makes the reviewer's loaded-machine scenario unlikely.

#### Checks

- Run now, on the rebased tree with the landing fixes:
  - `cargo fmt --all -- --check`;
  - `dprint check` on the two spec files;
  - `cargo clippy -p farhelm-helm --all-targets -- -D warnings`;
  - `cargo clippy -p farhelm --bins -- -D warnings`;
  - the full helm and protocol packages through the recorder (`cargo nextest run -p farhelm-helm -p farhelm-proto`,
    pinned tmux, four slots, no retries): 1213 passed, none skipped, run `077d1907-4be7-4dba-b758-fe30e6206279`. One
    test's trace capture reported a single evicted trace event, a diagnostic note rather than a failure.
- Stopped on purpose: an earlier start of the same run, because the landing fixes were edited in while it ran.
- Reused: the executor's browser settings runs, asset JavaScript harness, desktop renderer compile, website build and
  sleep checker. The landing fixes touch only helm-side alert text, a log line, a migration's error context and spec
  wording, none of which those checks exercise.
- The package-wide helm run gives the clean pass the executor's report could not claim. Its own package run had failed
  on three fixtures it then fixed and retried only narrowly.
