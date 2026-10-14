### What this was about

Installed Farhelm apps had no way to learn that a release was withdrawn. The existing latest-release answer has a fixed
format, and a release's signed checksums cannot carry changing notices to apps already running. This adds a separate
signed channel that older clients can keep reading when later releases add information they do not understand.

The original plan also called for replacing a withdrawn update already installed and waiting for restart. Execution
found that a supervisor can migrate local state and fail before recording its version, so an old or missing version
record cannot prove an older replacement is safe. The maintainer chose the narrower release: warnings and refusal to
install withdrawn targets, with manual recovery for a withdrawn waiting update. The separate Near term follow-up stays
unchanged.

### Things you should know

The running release gets a warning and the maintainer's reason once an update check learns its withdrawal. A safe newer
release can still install normally. A safe update waiting for restart keeps its red arrow, even while the running
release carries a withdrawal warning. Both automatic and on-demand checks refuse a withdrawn installation target.

A withdrawn update already waiting for restart loses the arrow and Restart to update offer. The hover says manual
recovery is required before restarting, and neither kind of check installs over it. An ordinary quit and relaunch can
still open that withdrawn version; this work cannot promise automatic recovery or choose a safe replacement for it.

Automatic updates still check release information about daily while an update waits, but do not install another one.
Turning them off prevents new background advisory requests, including a signature request after a body request already
started; an explicit check or a request joining existing work can continue. No extra background fetch occurs while off.

Only a complete authenticated snapshot replaces the information held for this app run. Missing or failed downloads
retain the last verified withdrawals; a later verified file can lift them. Nothing is persisted across relaunch. Unknown
directives are ignored. Advisory failures never create the reinstall warning or block an otherwise permitted update.
Withdrawal and reinstall warnings share one mark, with both reasons in the hover.

The maintainer's external publishing tooling must add `/advisories` and `/advisories.minisig` at get.farhelm.io. The
UTF-8 file starts with `farhelm advisories`, followed by `revoked vX.Y.Z reason` lines for stable tags. Sign the
complete file with a prehashed minisign signature, a key installed clients trust, and trusted comment exactly
`farhelm advisories`; publish the matching pair. The body cap is 64 KiB, signature cap 4 KiB, known directive-line cap
4096 bytes and displayed reason cap 240 characters, with controls removed. The publication instructions in
releasing/AGENTS.md cover lifting withdrawals, future directives and rotation. Nothing is live until a file is
published; absence means nothing to say. No publishing tooling, production signing, site, installed app, supervisor
startup or version-record meaning changed. The standalone installer, helm and remote hosts do not read advisories;
replay protection remains outside this scope.

The fetch deadline now also bounds returning to the updater worker after HTTP cancellation. An already-running system
DNS resolver cannot be cancelled and may retain its thread until the OS call finishes; it owns no updater state and
cannot hold the worker's subsequent checks. A controlled blocking-resolver fixture exercises this shutdown boundary.

### Open questions and possible follow-ups

Automatic waiting-update recovery remains the existing Near term follow-up. It needs durable attempted-version evidence
before migration, conservative treatment of old or interrupted records, and review of commands running inside sessions
under that new meaning. This plan neither changes nor weakens that deferred requirement.

The external publisher needs the new two-file operation before withdrawals can reach these clients. Only test-key
fixtures were signed here; the private test key was generated nonextractably in a short-lived process and never saved.

### The PRs

- [#1991: shared signature verification](https://github.com/scode/farhelm/pull/1991/changes), an inert prerequisite.
- [#2010: withdrawal warnings and refusal](https://github.com/scode/farhelm/pull/2010/changes), the coherent feature,
  based on #1991. Both PRs remain drafts.

### Checks run, reused and skipped

Recorded runtime commands use pinned nextest, pinned tmux, four slots and zero retries in an owned bounded Linux
container. All commands ran from the checkout root. Evidence remains in the plan's working log beside the checkouts.

- Run:
  `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'desktop updater advisory decisions, parser, transport and readout' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-ui --features desktop -E 'test(desktop::updater::) | test(app_updater::)'`;
  run `efbf1890-77b1-4d59-88bc-6ae7bb47eb00`, 83 passed, 537 excluded. This establishes the
  optional-fetch/authentication/parser, trigger/privacy, manual-recovery, retained/lifted-warning, real HTTP and readout
  behavior before review corrections.
- Run: the same recorder and package with selection
  `'advisory source-review corrections: shutdown, privacy, statuses and malformed directives'` and filter
  `test(a_blocking_resolver) | test(disabling_automatic_updates_stops_new_advisory) | test(statuses_caps) | test(malformed_directives)`;
  run `66271aab-caa9-4698-ba28-f636850b4e28`, 4 passed, 618 excluded. This covers synchronous DNS shutdown, permission
  changes during both request phases, final 3xx refusal and independently effective malformed-line assertions.
- Run:
  `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'desktop Rust targets after advisory corrections and custom-permission rebase' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-ui --features desktop`;
  run `7d6b7ff9-7895-45e9-8800-005d2d85ebe1`, 623 passed, none skipped. No runtime `SKIPPED` messages. This covers the
  complete affected desktop package on the rebased tree; only the warning-glyph test was subsequently strengthened, with
  production code unchanged.
- Run:
  `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'withdrawal-only and coexisting warning glyphs' --concurrency '4 nextest slots; one selected test; retries 0' --tmux required -- cargo nextest run -p farhelm-ui --features desktop -E 'test(=app_updater::tests::the_warning_mark_survives_a_waiting_update)'`;
  run `4afb56c0-8eec-4cf5-8fed-ee44940af270`, 1 passed, 622 excluded. Exact vectors cover warning-only, safe-update
  arrow and coexistence after the source review found that gap.
- Run: `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` with the isolated pinned parser interpreter, targeted `dprint check` for the
  changed Markdown, and `python3 releasing/check-changelog.py format`. Final targeted Clippy covers the corrected
  desktop implementation and shared rendering; the delay check reports 298 delays, none missing a rationale.
- Run: `cargo check -p farhelm-ui --features desktop` before the correction round. The later desktop all-targets Clippy
  and recorded package build cover the corrected implementation too; another duplicate compile check adds no evidence.
- Reused: the package and mark-test execution across the final rebase from main
  `727d23b8371dbbedcb545588dc9619fc107713c7` to `8b9323ddbb7e7b510ed88d2c2e0d3cc23881fb95`. Read every incoming diff.
  The new help-menu release-notes item changes menu ordering and opening that page, not readout eligibility, marks,
  updater dependencies or signatures; bookkeeping and instruction changes do not affect behavior. Final desktop
  all-targets Clippy passed on the combined tree. No runtime repetition is warranted for those independent changes.
- Reused: the foundation's final release-authentication run `e4b7fa6e-2e6d-472c-abda-8d45be811264`, 6 passed, 1068
  excluded, and its helm-lib Clippy, format, delay and source gates at original head
  `6da6cb7840a1277f7db00c18e797154adb48f41f`. Careful rebases preserve that three-file patch; incoming launch,
  preference and protocol work changes neither signature verification nor its trust/checksum callers.
- Reused: the prior source inspection of all stable tagged installers v0.23.0 through v0.28.2, nine tags and four
  distinct installer sources. They mechanically accept installation over a newer Installed version and the ownership
  record; those permanent contracts are documented. This is source evidence, not a runtime downgrade-safety proof, and
  no withdrawn waiting replacement uses it.
- Skipped: full workspace and browser batteries, hosted CI, desktop GUI smoke, installer runtime and release/live gates.
  The change is native updater logic and pure presentation, covered by the affected package and focused transport/state
  tests; no installer, web flow, desktop entry point or release artifacts changed. No new executable documentation
  examples need doctests.

No updater or desktop runtime test failed in this resumed run. The first desktop compile caught a wrong verifier export
name, corrected before testing. One recorder launch refused output-root permissions before spawning or creating run
evidence; permissions were corrected. One targeted Clippy run rejected a complex callback type, fixed with a borrowed
alias. These are setup/compile/lint failures, kept distinct from runtime passes. Earlier executor's failed safety-probe
setup run and later successful migration proof remain retained separately; neither is rewritten as a pass.

### Review gate outcome

A required fresh gpt-6.1-sol reviewer at high effort found four concrete issues: the synchronous fetch could wait for
blocking DNS after the HTTP deadline; disabling automatic updates mid-probe still began advisory requests; final 3xx
responses were accepted as bodies; and a later duplicate hid several malformed-line test failures. All were corrected
and independently rechecked. That recheck found one additional warning-mark coverage gap, corrected by the exact-vector
matrix and its recorded focused proof. The final fresh gpt-6.1-sol/high scope and source review found no actionable
findings or unnecessary implementation complexity. It confirmed the narrowed recovery decision, defensive boundaries,
test effectiveness and the documented resolver/relaunch limitations. Source reviews did not claim runtime execution.

The existing foundation's independent source gate is reused after preserving its patch through careful rebases.
Documentation was separately inspected across all touched files. Required commit/PR wording cold reads accepted the
second draft after replacing unexplained version-record jargon. Report delivery also has a separate own-model native
cold read for product understanding and public-repository hygiene.

Implementation and investigation stayed local under no-workhorse. Only required resume/source/scope/wording/report
reviews were delegated. Private galaxy-brain session records retain the routing and outcomes; native and orchestrator
usage are unavailable, and unverified external counter semantics are excluded from totals.
