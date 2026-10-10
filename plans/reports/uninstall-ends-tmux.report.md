## What this was about

Uninstalling Farhelm through a remote host's menu left its private tmux server running after removing the executable it
used. Keeping that server alive also made removal depend on systemd remembering a process-only termination policy. The
maintainer decided that the private server is an installation detail and should end with removal; local
`farhelm uninstall` stays unchanged.

## Things you should know

Remote removal now disables the service, removes its unit file, reloads systemd, stops the supervisor and its private
tmux server, removes the private executable directory, and finally forgets the host. The host's Farhelm data stays in
place.

Planning and confirmation still refuse live sessions or terminal tabs. A session or tab started during removal may be
ended, as agreed; no additional session check was added.

Execution found a retry case the planned reorder alone did not cover: if the supervisor had already stopped under its
old policy, tmux could survive after systemd forgot the service. The existing stop step therefore also ends the server
through the host's exact private socket, using the client and socket frozen at confirmation. This can reach that named
server outside the service's process group. It does not search for other servers or processes. Verified absence permits
retry after the private executable directory is gone; permission, filesystem and client failures leave the host listed
for retry.

## Open questions and possible follow-ups

None required to complete this plan. The tests exercise Linux's user service manager and real SSH to a fixture host; no
macOS remote-host execution was performed. Local uninstall was outside the agreed scope.

## PRs

- [PR #1781](https://github.com/scode/farhelm/pull/1781/changes): remote uninstall ends private tmux, with the spec,
  retry coverage and triage bookkeeping in one commit on `plan/uninstall-ends-tmux/01-uninstall-ends-tmux`, change
  `vszrrtmmnypm`; delivered as a draft.

## Checks run, reused and skipped

- Ran focused uninstall and real-SSH provisioning tests through `scripts/record-test-run.py`:
  `cargo nextest run -p farhelm-helm --lib -E 'test(uninstall) | test(provisioning_and_update_over_ssh_preserve_an_operable_session)'`.
  All 15 selected tests passed in run `10746665-9437-4b2b-9972-73a849ccfac4`, including normal removal and the
  inactive-service retry. The actual user manager and SSH substrate ran; no runtime substrate skip appeared. The
  recorder verified nextest 0.9.143, tmux 3.7c, four slots and zero retries.
- Ran the four focused shell, frozen-plan, executable-directory-gone retry and confirmation-boundary cases in run
  `35614064-0a22-4111-9846-fc3f99797f98`; all passed. After the final filesystem-error correction, reran the exact shell
  case with
  `cargo nextest run -p farhelm-helm --lib -E 'test(=provisioning::tests::uninstall_host_commands_refuse_setup_units_and_skip_finished_work)'`;
  it passed in run `d75262ad-b101-437a-a777-ce1d3227cc2e`, including actual permission denial, missing client with
  absent socket, exact client absence diagnostics and injected I/O failure. No runtime subcase skip appeared.
- Ran `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-Markdown
  `dprint check`, `python3 releasing/check-changelog.py format`, and the isolated
  `python -B scripts/check-test-sleeps.py`; all passed. The delay checker inspected 278 delays with zero missing
  rationales. One ledger wrapping failure was corrected and its Markdown check repeated.
- Reused the real-SSH lifecycle evidence from the change based on `ccf98944` after the final absence-classification
  correction; that correction did not change active or inactive server termination, and the current-source shell run
  covers its error behavior. Reused the other focused results after the ledger URL amendment.
- Read the complete intervening main diffs and rebased onto `f58b97a8`. Initial changes were queue-only; main then
  gained the bounded supervisor stop-expiry quiet-down from PR #1780. It preserves ordinary shutdown and private-server
  identity, adding an independent attempt only when the stop budget expires. The remote removal order, paths and
  confirmation contract remain intact. The adjacent feedback-index deletions needed a mechanical merge preserving both
  removals. Existing ordinary-stop evidence still applies; no expiry-protection claim is made here, and no additional
  runtime run was needed for that interaction.

The isolated reproduction run `56977505-1c72-4157-ada3-8307b67cdf07` intentionally failed while testing the reorder-only
hypothesis: an inactive forgotten service could not be stopped and its private tmux server remained alive. That evidence
justified the socket fallback; it is retained as a failed hypothesis, not a flake or a successful validation run.

Skipped broad workspace, browser, desktop, installer and release suites: the change is confined to remote removal
planning and host commands, with direct shell, retry, confirmation and real-SSH coverage. No UI wiring or local
installer behavior changed. No live installation was modified.

## Review gate outcome

The prescribed fresh-context gpt-6.1-sol high reviewer identified the inactive-service retry gap and stale
documentation; both were corrected. A fresh scope reassessment approved cleanup through the exact private socket within
the existing stop step. The final source reviewer identified lookup errors being mistaken for absence; the existing
remote `stat` facility now distinguishes them. Final verification passed with no remaining findings. A separate
documentation pass covered every touched file.

The wording cold reader recovered the motivation, retry behavior and session caveat without contradicted claims.
Implementation and investigation were local; only the required reviews and cold reads used agents. Requested native
model selections are recorded, but actual runtime model identity and usage counters were unavailable. Private review and
delegation evidence remains in the session records.

### Landing

Landed on 2026-10-10 (UTC) as #1781 (remote uninstall ends the host's private tmux server), one squash commit on main.
Since the plan was based, main gained host-icons, claude-compaction-status and the supervisor's stop-expiry quiet-down
(#1780); the rebase was clean. host-icons adds an icon and a color to each host row, which forgetting a host now deletes
with the row; nothing else in them touches the uninstall flow.

#### Review before merging

The first reviewer got stuck in a hung command for hours and was stopped; a fresh reviewer that had not worked on this
round's plans then read the change against main by reading the code only. It found the remote commands safe: the tmux
server ended is only the one whose socket is this installation's own `tmux.sock` in the state directory the plan already
shows as kept, the tmux program and socket are frozen in the confirmed plan (a change between plan and confirmation
refuses), every value reaches the remote command through the existing shell quoting, and the user's own tmux server or
another installation's cannot be reached. Any failed step keeps the host listed, a retry repeats the steps safely, and
live sessions or tabs still refuse at planning and again at confirmation. Remote setup and uninstall only target Linux
hosts with a systemd user manager, so there is no macOS path.

#### Fixes made while landing

- A browser test of remote uninstall still expected the old step order (the supervisor stopped before systemd reloads),
  so it would have failed the next time anyone ran the browser suite; the executor ran no browser specs, and CI does not
  run them. The landing updated the expected order and its explanation.
- A new code comment said the supervisor is stopped first "so its orderly terminal shutdown runs". With the reload now
  before the stop, systemd stops the supervisor and its tmux server together, so that shutdown no longer runs ahead of
  tmux during uninstall; the comment now says so. The supervisor's stop documentation and SPEC_impl.md's account of the
  process-only stop policy now name remote uninstall as the one stop that ends the private tmux server on purpose.

#### Not changed while landing

- On a host whose `stat` is BusyBox's rather than GNU's or uutils', the stop step cannot recognize a socket that is
  already gone, so every uninstall attempt fails at that step and the host stays listed. Nothing is lost, and removing
  the host from the list still works, but such a host cannot be uninstalled through the menu. SPEC.md says nothing
  provisioning does is distribution-specific, so this is a gap worth a follow-up if such hosts matter.
- SPEC.md says each removed item is named by its path on the host, but the confirmation names the private tmux server
  without its socket path. Either the confirmation or the spec sentence should change.
- The hosts docs page still says uninstall never stops a session. That stays true for live sessions, which still refuse,
  but it does not mention that a session started while the removal runs may be ended, as you agreed.

#### Checks

- Run now, on the plan rebased onto main with the landing's fixes: `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the helm's unit tests in full including provisioning (run `b70a0c4d`,
  1018 of 1018), and through the recorder on Chromium and WebKit with pinned tmux 3.7c, one worker and no retries, the
  multi-host terminal spec's uninstall cases (run `f172f6b1`, 4 of 4).
- Reused from the executor: its real-SSH removal run against a fixture host with a systemd user manager, which ran
  before main gained the stop-expiry quiet-down. That change adds at most twelve seconds before a supervisor exits and
  then exits regardless, so it cannot hold up this stop; that was judged by reading, not re-run.
- Skipped: the CentOS provisioning container and macOS, which this flow does not target.

Nothing in the report above was made untrue by the landing.
