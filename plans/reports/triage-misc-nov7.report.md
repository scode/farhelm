## What this was about

Removing a host could be acknowledged before its deletion had the durability used for other trust revocations. A power
loss could restore the host with its old command and YOLO permissions. Host removal now uses the existing durability
barrier, and the implementation specification names that guarantee.

Separately, editing Custom permission words while a suspended host resumed could automatically launch the edited agent
without another Launch press. Those edits now cancel the pending press. The regression exercises an admitted edit
through the draft owner and the same identity used by the launcher's submission guard.

The other six outcomes carry out the recorded triage decisions: accept the existing right-edge Korean-sign ordering in
the specification, add two container-approval requirements to the Podman rigor TODO, and discard three findings confined
to old terminals. Each outcome has its own draft PR and execution record.

## Things you should know

The two fixes contain 37 and 99 added-plus-removed lines of code, tests and specification, within their 100-line gates;
the agreed ledger, queue and changelog exclusions apply. The launch fix adds Custom text to the existing identity rather
than introducing a separate edit-retirement mechanism. Unfinished text edits cancel too; the existing Resume connection,
automatic-name and checkout-configuration allowances stay intact.

The durability test checks that the host deletion runs under the full-sync setting and restores the ordinary setting
afterwards. It does not simulate a physical power loss. The existing macOS caveat still applies: these writes reach the
drive's cache.

The container-approval changes are recorded future work, not implementation. Launch cards must show container settings;
container-bearing cards must also use a kind older pages cannot decode, before or together with CLI container launches.

With wrapping disabled at the right edge, a Korean vowel or trailing sign printed after the last-column character can be
stored on the cell before it. Copied text then places the sign before that final character, and a later text or emoji
presentation selector still applies to the final character. Triage accepted this existing behavior; the program's input
and files are unaffected.

The three discarded findings concerned these conditional failures, assessed from source rather than observed incidents:

- A temporary startup socket-probe failure can make a live old session appear ended, so Delete stops its agent without
  the live-work warning.
- When tmux is unusable, a Delete presented as stopping nothing alive can still stop an old agent or old-tab programs
  beside an exited agent in a terminal host. The old-agent case then refuses after the destructive work; the other can
  complete.
- If a query client reports that the server exited while the server still lives, Farhelm can mistake that answer for an
  empty server. Startup can then destroy its old terminals; later listings can hide live work and allow Delete without
  the live-agent warning. Whether tmux produces that diagnostic while its server lives remains unverified.

Triage chose not to repair these old-terminal failures before removing that support. Their code remains unchanged. No
vendor code or live installation was changed.

## Open questions and possible follow-ups

The launcher's broader live-draft concern remains unverified. Its original generic investigation and later container
review remain in the feedback queue under Other; only the confirmed pending-Resume route was fixed. The two container
requirements remain in the existing Podman rigor TODO for later execution. No maintainer decision blocks this delivery.

## The PRs

All eight are open drafts in this order, each based on the preceding PR, with the first based on main.

- [#3171](https://github.com/scode/farhelm/pull/3171/changes): durable host removal, its specification and regression.
- [#3174](https://github.com/scode/farhelm/pull/3174/changes): Custom edits cancel a pending resumed launch; retain the
  generic feedback remainder.
- [#3175](https://github.com/scode/farhelm/pull/3175/changes): accept the existing right-edge sign and selector
  interaction.
- [#3176](https://github.com/scode/farhelm/pull/3176/changes): record container settings on launch cards as Podman work.
- [#3177](https://github.com/scode/farhelm/pull/3177/changes): record older-page container-card compatibility and its
  CLI ordering.
- [#3179](https://github.com/scode/farhelm/pull/3179/changes): discard the old-terminal startup-probe finding.
- [#3180](https://github.com/scode/farhelm/pull/3180/changes): discard the old-terminal Delete-consent finding.
- [#3181](https://github.com/scode/farhelm/pull/3181/changes): discard the old-terminal client-loss finding.

## Checks run, reused and skipped

- Passed: recorder-backed `cargo nextest run -p farhelm-helm --lib` selecting the full-sync trust-revocation test and
  host-removal tests. Run `68554e7c-5585-4446-bc88-56518fe21081`: four passed, 1,562 selected out.
- Passed: recorder-backed `cargo nextest run -p farhelm-ui --features desktop --lib` selecting the ordinary draft-owner
  tests and guarded/resumed submission-stamp tests. Run `304b6fb8-eb66-40cb-a82d-160ac1f29217`: four passed, 1,093
  selected out. Both runs used pinned nextest 0.9.148, four slots, zero retries and `--tmux none`; the tests use a
  temporary store or headless component state, not terminals. Both have complete source capture and runner reports,
  untruncated output through EOF, no runtime `SKIPPED` marker and no cleanup limitation.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, isolated
  `python -B scripts/check-test-sleeps.py` (423 delays, none missing a reason), changed-Markdown `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py` (zero errors, three existing
  unrelated citation warnings).
- Reused for the final drafts: the store run covered base `f164d321` plus the first fix's working diff; the UI run
  covered `a8cdef3c` plus the second fix's working diff. Subsequent edits only record PR URLs and the independent
  specification, TODO and queue outcomes. Executable and test content is unchanged. All intervening main diffs through
  `23d104ac` were read: plan bookkeeping and the OpenSSH path-refusal stack, including separate Tensorlake specification
  clauses, do not interact with the store barrier or launcher identity. No rebase or runtime repetition was needed.
- Skipped: other Rust suites, browser and desktop-application runs, installer, provisioning and cloud checks. The
  focused tests cover the changed persistence and pending-submission contracts; the other six outcomes change only
  documentation and queue bookkeeping, for which runtime tests are explicitly unnecessary. No broader coverage gap was
  identified.

## Review gate's outcome

Every PR passed its required single fresh-context gpt-6.1-sol review at high effort, with no actionable findings.
Separate cold readers checked the commit and PR wording; the independent resume check passed. No review finding remains
unresolved.
