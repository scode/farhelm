## What this was about

Adding or updating a remote host could hang indefinitely when the host allowed one SSH session per connection and
required approval for each new login. The upload occupied that session, so separate file-size checks failed and never
started the stall timer.

The maintainer chose to have the upload report the remote file size on its own session. This preserves the rule that
only growth on the host counts as progress, including while buffered bytes drain after the helm finishes writing. The
implementation removes the separate check and keeps the one-minute stall limit.

## Things you should know

The upload reports its file size every two seconds and once when its writer finishes. It consumes stdin in the
foreground under the host's POSIX shell, preserves upload failures, and stops its reporter when the command exits.
Malformed or oversized reports cannot renew the deadline; a closed progress stream does not cancel an already armed
deadline. Unit files use the same upload path and retain their digest checks.

Adding or updating a host can still wait indefinitely before the remote upload file is created if the SSH connection
never fails. Farhelm adds no timeout in that previously accepted window; the one-minute stall limit begins only after
the file exists and the upload reports its size. The work adds no local-byte progress, progress display, SSH keepalive,
or overall transfer deadline. The covered TODO entry is removed, the implementation specification explains the progress
source, and a changelog fragment describes the affected host behavior.

Two earlier focused runs each passed nine tests and timed out in the slow-setup/growth fixture. The exact test passed
once and then five isolated attempts, so the cause of those concurrent failures remains unproven. The fixture now gives
individual reports more scheduling headroom while keeping setup and total growth longer than its idle budget; the
corrected ten-test cohort passed. Production's one-minute limit is unchanged. These are retained current-session
development observations, not a new latent-flake entry.

## Open questions and possible follow-ups

No maintainer decision is needed. The no-second-command proof observes the real command-launch boundary through a
controlled held-open upload. It does not recreate an SSH server's one-session policy or interactive approval. The
separate CentOS check passed real SSH installation, session creation, reconnection and update.

## The PRs

- [#1728](https://github.com/scode/farhelm/pull/1728/changes) — same-session upload stall detection, regression proofs,
  documentation, changelog and covered TODO removal. It remains a draft, pushed at
  `fdcfd477cf49a5b80cff84f6937d31bf1e266c8e`; landing belongs to the monitor.

## Checks run, reused and skipped

Focused recorder run `c72ebd80-d62a-47e7-b128-83deca5369c7` passed all ten selected tests. It covers byte-exact
POSIX-shell input, prompt pipe closure, upload failure status, growth-only arming and renewal, stalls after
input/progress EOF, bounded malformed reports, descendant cleanup, temporary cleanup and retry, and no concurrent
command launch.

The full helm crate run `0586a436-5a3c-4ce2-b0f6-8307f2860ca6` ran 1,016 tests: 1,014 passed and two existing
probe-group reaping tests failed. That run remains a failure. Exact reproduction `494d0fdb-7e08-4b06-affd-75b0a9dca9dc`
failed both again. Process-state inspection showed the killed helpers had become zombies under the sandbox's non-reaping
PID 1; signal-zero presence checks therefore never saw the groups disappear. Running the identical two tests under an
owned child-reaping wrapper passed both in `7a4af91b-e9b1-41f0-800c-35bafe992cd9`, with no source changes. Reuse the
other 1,014 passes and this targeted correction: the identified gap was the sandbox's descendant reaping, so another
full crate run would add no evidence for the fix. Reports, source identities and output are complete. Three JUnit passes
were runtime early returns because the sandbox had no usable local systemd user manager: direct-local provisioning,
SSH-to-localhost provisioning and teardown-on-failure. Those passes do not prove that substrate. The real CentOS gate
separately supplies a systemd user manager and SSH host; direct-local runtime coverage remains unavailable here.

Recorded `scripts/test-provision-centos.sh` passed the real CentOS SSH provisioning/update test in run
`d085c3c7-ff18-4eef-ac8c-73bb27f01d47` (one selected test, 140.321 seconds). Its source identity, JUnit, output and
cleanup records are complete; no runtime SKIPPED message occurred. The gate exercised the script under the remote host’s
shell and retained real SSH/systemd readiness evidence.

Earlier CentOS attempts remain retained. The first stopped before runtime because the owned sandbox lacked `file(1)`;
the prerequisite was installed there. Run `a4e9eeb5-9c12-4d71-b92c-91fa91e6526a` reached real installation and
reconnection but exceeded nextest’s 300-second test timeout during update. Run `c09a5ca5-bd73-4429-b03d-00271a42aff1`
was deliberately interrupted after discovering Cargo had restored the original debug payload from its cached artifact.
Neither is a pass. The final run used a debug-symbol-stripped static payload, reduced from 366,074,080 to 96,030,904
bytes, with the ELF entry point and every allocated section’s metadata and content verified identical. Its actual
post-build input was checked against that verified artifact. Product source and runner policy were unchanged. The
smaller input passed; that observation does not establish all causes of the original timeout.

Both required Clippy configurations passed: `cargo clippy --all-targets -- -D warnings` and
`cargo clippy -p farhelm --bins -- -D warnings`. Their results are reused after only test timing literals, diagnostic
text and comments changed; production behavior and type structure remain the tested ones. Rust formatting,
changed-Markdown formatting, changelog format and the isolated source-delay checker passed; the checker inspected 270
delays with zero missing reasons. A separate documentation pass covered every touched file.

Carefully inspected every change on main between the original base `98e942f6` and `192a280e`: launcher-template
UI/specification/browser/docs, idle-CPU investigation documentation and TODO additions, plus plan goals, reports and
claims. None changes provisioning behavior, its specification section or its dependencies. The rebase applied cleanly
and preserved upstream TODO additions. Both changed Rust files and the fragment remained byte-identical to the
reviewed/tested source at `7d13e455`; changed-Markdown formatting, changelog format and whitespace checks passed again
after the rebase. Runtime evidence and source reviews are reused because that inspection found no affected behavior or
uncovered interaction.

The earlier failed records remain retained: `2b8c8af1-82a9-4b55-b4f4-67f32f77de99` and
`f08f5bad-2ad4-4cfb-8f49-b35e728d73aa` were repaired test compile errors; `9ce5c248-a793-4859-8b19-5f76c4e55ea1` and
`f2836cb4-8d8f-45ed-8d2d-b46b4474ea84` were the two nine-pass/one-timeout cohorts. Exact reproduction
`e2c82c53-5560-425c-ade3-10a23d6413bf` passed, and batch `c266fca6-3a16-4988-8638-1ef5531c0c1d` passed five isolated
attempts. Later passes do not erase or explain the failures.

Skipped full-workspace runtime, browser, desktop, installer and website batteries: this change is confined to remote
provisioning transfer supervision, and the focused proofs plus required helm/real-SSH checks cover the affected
behavior. No hosted CI or deployment was requested.

## Review gate's outcome

The required gpt-6.1-sol high reviewer inspected the full change with the plan's acceptance criteria and the verbatim
test-authoring checklist. Findings concerned test readiness, whole-pipe completion, atomic PID publication, GNU-stat
platform assumptions and process scheduling; all were addressed. Final source review and the later timing follow-up have
no findings. A fresh scope review found no unnecessary substantial mechanism. Reviewers ran no runtime tests; recorder
verdicts are executor observations.

Commit and PR wording passed a separate fresh gpt-6.1-sol medium cold read. Implementation stayed with the executor
under no-workhorse mode; only the required process reviews were delegated. Native model-reporting and usage counters
were unavailable, with that gap recorded in private galaxy session evidence.

### Landing

Landed on 2026-10-09 (UTC) as #1728 (upload stalls are detected on hosts that allow one SSH session per connection), one
squash commit on main.

#### What else was on main, and what lands with it

Between the commit the change was built on and the landing, main gained the repository-cache, desktop encoding and
conversation-warning changes (#1720, #1724, #1722, #1723, #1725) and #1730, plus documentation and the planning queue's
bookkeeping. None of it touches the helm's provisioning code, the protocol (beyond a comment) or a database schema. The
drag-copy-notice plan is landing in the same round; the two share only TODO.md and SPEC_impl.md, in separate entries and
sections, and merge without conflict.

#### Review before merging

A separate reviewer that had not worked on either plan read both against each other and main before anything merged and
found nothing that breaks. This change is the helm's upload of Farhelm to a remote host, not the terminal's attachment
upload, so the attachment tests are unaffected; nothing outside the two changed files used the removed size probe; and
SPEC.md describes stall-only timeouts without the mechanism, so it stays accurate.

#### Checks

- Reused: the report's checks. Nothing that reached main since the change was built touches code it changes or depends
  on.
- Skipped: running anything again during the landing, for the same reason.

Nothing in the report above was made untrue by the landing.
