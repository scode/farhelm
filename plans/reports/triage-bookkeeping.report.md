# Close discarded findings and preserve duplication work

## What this was about

The November 21 triage closed 45 review findings and set aside 17 findings about duplicated helpers for one near-term
cleanup. The two PRs remove those 62 findings from the open queue and archive their decisions. **Consolidate the helpers
duplicated across Farhelm.** in the Near term section of TODO.md keeps the 17 findings' locations and proposed
consolidations. For example, terminal word selection and file-path recognition each interpret the same grid cells. A
shared row walker could keep their handling of gaps and wide characters in sync.

## Things you should know

Several discards follow the maintainer's decision about updates that arrive just before the screen redraws. An ordinary
key or click can use the newly arrived information. Destructive confirmations and Replace with need to honor what the
user saw. For example, a folder history reply could clear the highlighted folder as the user pressed Enter. The launcher
could then start a session instead of choosing the folder. In Replace with, that could retire the source session. The
separate desktop triage plan owns the specification clarification and the Replace with guard.

The duplication TODO points to the current Start column and search renderers after the launcher redesign. The targeted
golden screenshot work changed how captures are selected and run. The remaining duplication is in how individual
screenshot scenes arrange their fake sessions and open dialogs.

## Open questions and possible follow-ups

No open decision. The duplication cleanup is tracked in Near term. The separate desktop triage plan tracks the launcher
clarification and Replace with guard.

## Pull requests

- Archived 45 findings the maintainer chose to discard. [#4465](https://github.com/scode/farhelm/pull/4465/changes)
- Archived 17 findings about duplicated helpers and added one Near term TODO entry.
  [#4466](https://github.com/scode/farhelm/pull/4466/changes)

<details>
<summary>45 discarded outcomes</summary>

- `shell-tab-label-omitted-row.md`
- `cleanup-retry-omits-recovery-step-times.md`
- `trailing-ui-retained-whole-environment-question-stale-unreachable-rationale.md`
- `group-divider-can-change-shift-sweep-range.md`
- `mute-menu-expiry-preview-can-go-stale.md`
- `shift-sweep-can-mark-through-sidebar-clip.md`
- `session-sign-in-suppresses-connection-diagnosis.md`
- `new-host-attention-cue-can-be-lost.md`
- `late-tab-restore-hides-upload-notice.md`
- `retired-connection-leaves-suspend-progress-stuck.md`
- `settings-search-key-hidden-by-open-editor.md`
- `trailing-podman-environment-d-drops-explicit-empty-proxy.md`
- `snapshot-cleanup-races-new-host-registration.md`
- `package-name-trailing-minus-requests-removal.md`
- `recipe-destination-refuses-absolute-dns-name.md`
- `apt-regex-fallback-can-accept-package-typo.md`
- `trailing-shared-recipe-container-base-summary-raw-presentation.md`
- `filtered-tab-reset-can-route-input-before-display.md`
- `checkout-use-default-presentation.md`
- `upload-path-pastes-into-restarted-agent.md`
- `wayland-drop-lost-before-file-list-arrives.md`
- `sidebar-sign-in-hides-launch-refusal.md`
- `add-host-close-reads-live-sign-in.md`
- `provisioning-waits-use-fleet-slots.md`
- `tunnel-control-exit-can-open-new-ssh-connection.md`
- `cancelled-ssh-capture-can-retain-local-readers.md`
- `automatic-update-opt-out-can-roll-back.md`
- `trailing-podman-place-typed-ssh-refusal-reasons-missing-final-ui-escaping.md`
- `antigravity-historical-restart-can-target-child.md`
- `mac-uninstall-unresolved-loaded-plist.md`
- `shell-idle-plays-agent-turn-finished.md`
- `quick-switcher-enter-can-run-a-different-template.md`
- `deferred-template-changes-unpresented-launch.md`
- `launcher-catalog-moves-model-highlight.md`
- `desktop-path-handoff-exceeds-linux-entry-limit.md`
- `macos-shift-sweep-can-miss-release.md`
- `repeated-resizes-clear-missing-history-notice.md`
- `empty-history-clear-keeps-missing-history-notice.md`
- `restored-capped-title-loses-boundary-whitespace.md`
- `reconnect-replay-keeps-obsolete-event-count-fence.md`
- `mixed-input-acknowledgements-misattribute-written-bytes.md`
- `snapshot-state-races-build-admission.md`
- `desktop-relaunch-sleep-depends-on-path.md`
- `linux-second-launch-lost-during-quit.md`
- `desktop-ignores-empty-login-path.md`

</details>

<details>
<summary>17 outcomes deferred to the TODO entry</summary>

- `desktop-startup-update-duplicates.md`
- `terminal-grid-row-traversal-duplicated.md`
- `gui-controls-repeat-shared-rendering.md`
- `gui-focus-checks-repeat-drawn-predicate.md`
- `approval-views-repeat-status-lookups.md`
- `gui-command-paths-repeat-reply-classification.md`
- `hosts-menu-and-settings-repeat-operation-logic.md`
- `launcher-repeats-choices-and-summary-formatting.md`
- `gui-screenshot-scenes-repeat-fixture-wiring.md`
- `session-list-actions-repeat-admission-and-progress.md`
- `session-pane-repeats-last-known-status.md`
- `settings-repeat-preference-and-focus-wiring.md`
- `templates-repeat-lookup-and-control-wiring.md`
- `terminal-repeats-modifier-mapping.md`
- `trash-repeats-animation-and-focus-wiring.md`
- `control-tests-repeat-simulated-key-events.md`
- `session-list-tests-repeat-frame-and-request-helpers.md`

</details>

## Checks

- `dprint check` passed on the changed Markdown. `python3 scripts/check-agents-md.py` passed with one existing
  historical citation warning in the archive.
- `deno test -A <scratch verifier>` passed both focused tests of the verifier. Running the verifier on each draft
  confirmed that the index matches the feedback files on disk, each selected decision appears once at the end of the
  archive with its draft PR link, and the other decisions and earlier archive text are intact.
- All checks were run for this plan. Runtime suites and CI were skipped because the changes are Markdown bookkeeping.

## Review

The source review for #4465 checked all 45 decisions and confirmed that the draft carried out the agreed removals. The
source review for #4466 checked all 17 decisions and their detail in the single TODO entry. Its findings led to putting
the entry in the general Near term list, distinguishing the screenshot runner from the remaining scene fixture work, and
pointing the cleanup at the current launcher renderers. Both drafts also passed commit-message cold reads.
