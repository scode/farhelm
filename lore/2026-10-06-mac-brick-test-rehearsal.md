# Bring-up rehearsal: v0.23.0 → v0.24.0

NOTE: Historical artifact from the 2026-10-06 rehearsal; frozen and not maintained.

Verdict: **pass for the maintainer-selected Codex-only baseline**, on 2026-10-06. This is an after-publication rehearsal
against these two releases. The next release needs a new run against its actual published bytes before promotion.

Candidate v0.24.0: `e69739faab9cad475fb39ec383ebff207ac20826`. Previous stable v0.23.0:
`f8dcc540fcbd751363f672294c5124b530ef249a`. Candidate SHA256SUMS SHA-256:
`4e77255c5923266b44c9ec3c087eb2bcab1ac61bb318aeb775e4505205be0b40`. The `/latest` endpoint still named v0.24.0 when the
path was selected. Automatic updates were disabled before the old app's first launch; the trigger was its normal **? →
check for updates** action, followed by **restart to update** in the version readout's menu.

The two passes used separate fresh ARM64 macOS 26.6.2 / Ubuntu 24.04.5 LTS pairs cloned from stopped private bases with
no Farhelm binaries or state. Guest-to-guest SSH used a guest-owned key and strict host-key pin. Default network peer
isolation required reciprocal guest /32 routes through the shared gateway; host forwarding was already enabled and was
only inspected. All Farhelm installs, updates, authentication, permissions and product interactions occurred inside the
guests. The host's live Farhelm was untouched.

## Functional evidence

Screenshots, raw logs and receipts remain private. Labels below refer to the exported fresh/upgrade evidence bundles;
they are not published attachments.

| Pass / check                          | Expected                                                           | Observed                                                                                                                                    | Result | Evidence labels                                                                                   |
| ------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------- |
| Fresh / public installer              | Canonical native app and CLI at candidate                          | Public v0.24.0 installer; CLI 0.24.0                                                                                                        | pass   | install.log                                                                                       |
| Fresh / native startup                | Usable app and connected local supervisor                          | App/local supervisor 0.24.0; session list usable                                                                                            | pass   | fresh-01-app-ready, first-launch.stderr                                                           |
| Fresh / Linux provisioning            | Scratch provisioning through GUI; connected candidate supervisor   | All setup steps complete; remote 0.24.0, systemd user service active; static tmux 3.7c                                                      | pass   | fresh-02-hosts-connected, service-verified.txt                                                    |
| Fresh / authenticated agent           | Genuine selected CLI replies and captures a resumable conversation | Codex 0.160.0 replied; hook capture matches actual saved thread metadata                                                                    | pass   | fresh-03-codex-reply, session-identities.json                                                     |
| Fresh / remote terminal               | Plain command input produces live Linux output                     | GUI-launched bash returned the submitted marker and another prompt                                                                          | pass   | fresh-04-linux-terminal                                                                           |
| Upgrade / old-state premise           | Old app and both supervisors before intentional update             | App/local/remote 0.23.0; automatic updates off; recorded conversation, template and compact preference                                      | pass   | upgrade-01-old-app, upgrade-02-template, upgrade-03-codex-marker, state-before.json               |
| Upgrade / continuity premise          | Original remote workload alive and advancing independently         | SSH observations agree on PID/start ticks/boot ID; counter advances                                                                         | pass   | upgrade-04-counter-before, continuity-initial.json, continuity-before-update.json                 |
| Upgrade / updater and relaunch        | Genuine old updater installs and its helper reopens candidate      | Installed 0.24.0; visible restart action; normal helper reopened app and connected local supervisor at 0.24.0                               | pass   | upgrade-05-restart-ready, upgrade-06-candidate-local, old-launch.stderr                           |
| Upgrade / stored state                | Existing sessions, template and changed setting remain usable      | Both sessions accessible; template and compact preference match original stored values                                                      | pass   | state-after-app-update.json, state-final.json, upgrade-11-reopened-template                       |
| Upgrade / genuine conversation resume | Existing agent responds; saved conversation restarts with memory   | Existing Codex replied; Farhelm Restart resumed the same thread and recalled the original marker                                            | pass   | upgrade-08-existing-codex, upgrade-09-resumed-codex, session-identities-after-resume.json         |
| Upgrade / remote supervisor update    | Connected candidate supervisor; original workload survives         | GUI update completed; remote 0.24.0 active; same original process identity and advanced counter                                             | pass   | upgrade-07-remote-updated, service-final.txt, continuity-after-remote-update.json                 |
| Upgrade / ordinary quit and reopen    | Candidate reconnects with state and live sessions                  | Native Quit exit verified before ordinary bundle reopen; app/local/remote 0.24.0; new agent input replied; original remote process advanced | pass   | upgrade-10-reopened-agent, upgrade-12-final-versions, reopen.stderr, continuity-after-reopen.json |

No manual candidate launch, supervisor kill, state reset or guest reboot was used to conceal a failure. The captured
conversation descriptor and actual Codex metadata retained the same thread identity across Farhelm's session restart.
The independently observed remote workload survived both updates and the final ordinary app quit/reopen.

## Addendum and validation

This release changes provisioning to use a separate SSH connection. The rehearsal exercised provisioning from scratch
and updating a connected old host with ordinary key authentication. Both tags have protocol 43, helm schema 42 and
supervisor schema 27, so this pair does not exercise schema migration or incompatible protocol recovery. v0.23.0 is the
earliest stable in the stated upgrade contract and the only earlier eligible stable for this candidate. No feature data
was agreed disposable across this upgrade.

The complete 27-test portable harness suite passed on actual Linux, including real-process continuity and refusal of
stopped/replaced workloads, ownership/deletion fences, shutdown transport failure and commented trust-array entries.
Recorder run: `ae57ffc0-e6dc-49c1-bb54-d399c9b4d5d6`. Earlier failed diagnostic/test-launch attempts were retained and
corrected; they were not counted as successful tests. No Rust or browser source changed, so their full suites were not
run for these Python helpers and documentation.

Both published installers were separately verified inside macOS against v0.23.0's compiled key ring, with exact signed
version comments and signed installer hashes. The corrected parser was rechecked against the actual previous-tag source
and recognized exactly the same two verified keys. Separate verification supports the receipts; the upgrade still used
the genuine old app's own updater.

## Limitations

Claude and other agent CLIs were not selected or tested. Neither stable tag carries the staged-candidate override, so
native override forwarding/removal and the manual pre-promotion candidate path remain deferred. Interactive second
factors, single-session SSH servers, Intel Macs and later schema/protocol changes are outside this run's coverage.

The original app launch's stderr captured updater/install/quit activity. The actual helper-relaunched desktop had FD 2
pointing to `/dev/null`; its stderr is unavailable. A later deliberate ordinary reopen used fresh captured stderr. This
is a diagnostic coverage gap, not a failed functional check.

Computer-use text delivery, native menu selection and modifier shortcuts were inconsistent. Physical character key
events and supported host search worked; visible scrollbar dragging worked where wheel scrolling did not. A native Quit
needed keyboard selection and a later process-exit observation. These limitations are documented without claiming an
isolated product cause. Guest service diagnostics also needed the actual systemd runtime environment in the direct Tart
command channel. The lifecycle helper was corrected to verify stopped state after macOS shutdown closes that channel
before replying.

## Cleanup

Successful pass evidence was exported privately before teardown. Disposable clones are removed through their ownership
manifests; prepared bases remain stopped and private for the next run. The operator's external journal retains exact
storage locations and archive hashes so the evidence and private bases can be found and cleaned up later. Raw evidence
and authenticated VM disks must never be committed or pushed to a registry.
