# Remote Apple-silicon Mac hosts

## What this was about

Adding a remote Mac previously required starting its supervisor by hand. The helm can now set up an Apple-silicon Mac
over passwordless SSH, update it and uninstall Farhelm through the ordinary host actions. Setup shows the program, data
and background-job paths before confirmation. Discovery still adopts a supervisor already running there without
restarting or replacing it, including when a new helm has lost its list of hosts.

The implementation uses a per-user launchd job and the existing signed Darwin release payload. Remote Macs do not
require, install or use tmux. Sessions, their terminal hosts and terminal-tab shells survive Update and a supervisor
crash. Uninstall removes the job and program, confirms terminal hosts stopped, and keeps the named data directory.

## Things you should know

On the sandbox Mac running macOS 26.6, the job did not return after a reboot and SSH reconnection, including a further
30-second observation window. Running setup again restored it. Setup and the docs therefore tell users to choose
**Update** in the host menu after reboot. Automatic startup at screen login is unverified; no automatic login or system
policy was configured.

Setup, Update, Update all, automatic updates and uninstall refuse a Mac with the Farhelm desktop app installed in either
ordinary Applications location. Discovery can still use its running supervisor. Intel Macs have no payload. A missing
command needed by the Mac path produces a refusal naming that command. The desktop app's own local supervisor and tmux
requirement are unchanged.

The specs and the **Add a remote host**, **Manage hosts** and **Move sessions to new terminals** docs now describe these
boundaries. The completed remote-Mac TODO entry is removed; the separate desktop-local launchd follow-up remains.

The sandbox SSH gateway masked remote command exit statuses. Actual lifecycle acceptance therefore used a private native
SSH endpoint on the same Mac. Product transport and its positive proof that no supervisor exists were kept unchanged.
The owned endpoint, jobs, processes, build trees and fixture data were removed after validation; the empty LaunchAgents
directory created for the reboot measurement was removed too. The Actions runner was left alone.

## Open questions and possible follow-ups

Screen-login startup needs a separate measurement before promising automatic recovery at that boundary. The existing
desktop-local launchd TODO remains a separate change after remote Macs have proved stable. The explicit ignored reboot
fixture is retained so future changes can repeat the measurement with operator authorization.

## The PRs

- [#3213: set up Apple-silicon Macs over SSH](https://github.com/scode/farhelm/pull/3213/changes) — setup, Update,
  uninstall, payload selection, tests and changelog fragment.
- [#3214: describe remote Mac host setup](https://github.com/scode/farhelm/pull/3214/changes) — specs, docs and TODO.

## Checks run, reused and skipped

The final code is `a9b6c902`; documentation is `bbdda115`. Runtime-tested code `d7f50524` differs only in explanatory
reboot-recovery wording and the changelog. The stack was carefully rebased onto main at `ec2e4984`. The incoming
prepared-snapshot work moved common installation execution and added another provisioning caller. The rebase retained
that extraction and the Mac action dispatch, and migrated the new caller's tests to the combined payload target and
inspected launchd domain. Both plans' spec changes were preserved.

Run against the integrated tree:

- `cargo test -p farhelm-helm --lib --no-run` and `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- Recorded `cargo nextest run -p farhelm-helm --lib` with a filter for native Mac contracts, payload sources and the
  interacting prepared-snapshot paths: 70 passed, no failures. Run `6240ce49-4d8b-4ce5-b54a-6cb8eeaa3eb2`.
- Same-tree Darwin payload build passed, run `e802dfed-100e-43c5-ad00-df3e61de9630`. The real Mac lifecycle then passed
  setup, agent and tab input, Update, crash restart, existing-host discovery, discovery from a fresh helm, and uninstall
  with data retained. It checked the same four process identities and fresh post-replay shell input across each
  disruption. Run `f971ac35-7bef-488a-bdc2-73cdf564bf1d`: one passed in 72.4 seconds.
- `cargo fmt --all -- --check`, scoped `dprint check`, changelog format lint and the test-sleep source checker passed;
  the latter found 430 annotated delays and no unannotated ones. The agent-instruction checker reported no errors and
  one pre-existing warning about an unrelated triage heading.
- The website build passed for 36 pages, including validation of every internal link and the new recovery-action link.
  It emitted empty-i18n/404 content warnings, with no build or link errors.
- Owned-resource audit and guarded cleanup passed, runs `43f1d5f6-7c46-4535-ac93-c1a0f1006545` and
  `243488f8-f02d-4cfe-abe6-82ce3c3717ca`. Both final nextest runs have complete reports/source capture; the build,
  acceptance, audit and cleanup records completed without forced cleanup or a cleanup limit.

Reused from the pre-rebase implementation on claim base `a24da7d0`:

- Linux provisioning selected 329 tests: 327 passed, while two real-host cases failed because the staged test binary was
  stale, run `3ff6f5ba-68c1-4750-9801-da781a264671`. After rebuilding that owned binary, both exact cases passed with
  local and SSH substrate exercised, run `e5f03a10-4564-418e-8acd-db034c943b12`. The rebase preserved their Linux action
  bodies; the new prepared-host interaction received the fresh focused coverage above.
- The real CentOS provisioning case passed with static musl and pinned tmux payloads, run
  `4fdd40a2-ed98-42f2-b40e-c43e2bd5c31b`, outer harness `ee83f76f-5ad1-4650-ab15-de05fbf98d72`. A private copy of the
  harness narrowed only its Cargo build to the helm library; container, SSH, PAM, systemd and lifecycle assertions were
  retained. This covers the shared portable chmod spelling, unchanged by the rebase.
- Authorized real reboot and setup recovery passed, run `76abde25-b5b8-4bc1-b76f-a95fd2d92b78`. The plist and startup
  policy were unchanged by the rebase, so another reboot would add no evidence. Published v0.37.1 Darwin payload
  download, verification and SSH-upload execution also passed, run `9e6816c2-294a-4fe6-8f2a-af1d38560c05`.

Earlier failed observations remain retained. Native execution exposed an incompatible BSD chmod argument order,
corrected before the successful lifecycle run. A first survival assertion lacked enough diagnostics to establish its
cause; the fixture now labels each phase and waits only for provisional unknown status, while definite ended states
still fail. A later run reached all initial survival phases but failed creating the fresh helm's database parent; that
fixture error was fixed. These are development failures, not evidence of latent flakes. One compile/test attempt was
interrupted during a disk alert and is not counted as a pass. The retained runs are
`3bf08dd5-b1e6-458f-81b9-c5d68560ad3b` (chmod), `9f43c83a-8baf-4366-a516-2dd11b189ad3` (unresolved survival assertion),
`5e9d6f7d-07e0-4386-ae3b-921d534eb764` (database parent) and `20a97c3c-e618-4dc7-b259-c88097ad822d` (interrupted
compile). Successful retries do not erase these observations.

No full workspace, browser or cloud battery was run: native host lifecycle, targeted shared provisioning coverage and
the retained Linux runs cover the changed behavior without unrelated breadth. The final corrections changed explanatory
text only, so the recorded native and Linux runtime evidence was reused; formatting, fragment lint and website build
were rerun. No preview server was started in this unattended run. Hosted CI is manual-only and was not dispatched.

## Review gate outcome

Each PR passed its single fresh gpt-6.1-sol review at high effort. The implementation review found no actionable issues.
The documentation review found an unavailable recovery-menu label and an outdated universal statement about Mac
supervisors; both were corrected, including the matching setup-plan and changelog wording. Each original reviewer
checked its corrections and found no remaining issues. Both PRs remain drafts; the executor has neither marked them
ready nor merged them.

### Landing

Landed alone in its round. Since the stack was based, main had gained: the per-host permission for agents to manage
sessions on their own host (own-host-sessions-without-asking), lower memory use when a terminal's saved snapshot is
restored (snapshot-in-place-paint), a correction to the OMP agent docs, and test and tooling changes. Saved Tensorlake
sandbox templates (prepared-snapshots) were already in the base, and the executor's rebase handled them. A fresh-context
review read the plan's diff against all of that and found no functional conflict. A Mac set up over SSH goes through the
ordinary add flow, so it starts with all five host permissions off, like any other new host. The plan constructs no host
record directly, so the new permission needed no change in its code.

#### Fixes made at landing

- SPEC.md still said "Installing Farhelm still requires a supported tmux", which the plan's own Topology text now
  contradicts: setting up a remote Mac over SSH never installs or uses tmux. The sentence now names that exception
  (#3214).
- A Mac with the Farhelm desktop app installed is still offered Update, Update all and automatic updates, and then
  refused with no way forward. The refusal message, and the Manage hosts docs page, now say to update Farhelm from the
  desktop app on that Mac (#3213, #3214). The changelog entry already says such a Mac is not supported and needed no
  change. The new message was checked by the helm's tests, not on a real Mac.
- On the Manage hosts docs page, the host-settings screenshot's description said "four permission switches". It was
  stale since the own-host permission landed, and now says five. "Your Mac's destination cannot be changed" is ambiguous
  now that remote Macs are hosts too, so it now says "the local Mac's destination" (#3214).

#### Gaps the report leaves out

- Uninstall on a remote Mac reads the Mac's mount table to check that nothing is mounted inside the directories it
  removes: when planning, and again just before it removes Farhelm's program directory, after the background job is
  gone. The parser refuses the whole table when any line is ambiguous. So one unrelated mounted volume whose name
  contains " (" or " on " (say `/Volumes/Backup (old)`) makes uninstall refuse. Nothing is removed that should not be.
  If the refusal comes at the second check, the Mac is left with Farhelm's background job removed and its program
  directory still in place, and the host stays listed with the remaining steps. Unmounting that volume and choosing
  uninstall again finishes it. You asked for this in conversation during this landing, so it is now a Near term TODO
  entry, added by #3228. It was not fixed at landing because the parser change needs tests of its own.
- To tell "no background job" apart from launchd failing, setup, Update and uninstall match the exact message and exit
  status seen on macOS 26.6. On a macOS release that words it differently, all three refuse and show launchd's own
  output, not a plain explanation. That is safe, but the error will be hard to read.
- The two new real-Mac tests (one of them reboots the Mac) are ignored by default and gated by environment variables.
  `.agents/validation.md` does not list them, so a later agent has to find them in the code. I left that as is.
- The README introduction, the docs Overview (which mirrors it, and is frozen until you ask for a change) and the
  desktop app's welcome screen still describe remote hosts as Linux machines.

#### A correction to the report

The report and the docs say to choose Update after a remote Mac reboots. The executor's real reboot test recovered by
running setup again, not Update. Update ends with the same launchd start, so it should work the same way, but that is
inferred, not measured.

#### Checks

Run on the stacked tree before the landing fixes (`35bddf0d`), on this development machine:

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format` and the test-sleep checker
  passed.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` (the
  shipped binary, which includes the helm) passed.
- Recorded `cargo nextest run -p farhelm-helm` (the whole helm crate, including provisioning against a local and an
  ssh-to-localhost host): 1642 passed, run `f6d22f75`. The only two skipped tests are the plan's ignored real-Mac tests.
- The website build passed (36 pages, internal links checked).

After the landing fixes (one refusal message and documentation wording): formatting, `dprint check`, clippy on the helm
crate, recorded `cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::/)'` (340 passed, run `e45fc76e`) and
the website build all passed. Main then differed from the tested tree only by those fixes and the TODO entry above.

Reused from the executor's report: the real Mac lifecycle on an Apple-silicon Mac (setup, Update, crash restart,
discovery from an existing and a fresh helm, uninstall keeping data), the authorized reboot measurement, the CentOS
provisioning run, and the run that downloaded and verified a published release's Mac program and uploaded it over SSH.
Main's changes since the executor's base touch none of the Mac paths. The CentOS harness gained only cleanup of
containers left by killed runs.

Skipped: browser tests (no change to the web UI's code; the new setup steps reach it as plain text) and the rest of the
workspace's Rust tests (the plan's code changes are all in the helm crate, and main's own changes were tested when they
landed). Hosted CI is manual-only and was not dispatched.
