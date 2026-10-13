# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`codex-upgrade-replays-legacy-selection.md`](codex-upgrade-replays-legacy-selection.md) — Pending reports spanning an
  offline upgrade can restore a discarded Codex conversation and send Restart to it.

- [`installation-identity-escape-collision.md`](installation-identity-escape-collision.md) — Distinct peer-chosen
  installation identities can display identically in an adoption prompt.

- [`continued-unit-line-bypasses-service-type-guard.md`](continued-unit-line-bypasses-service-type-guard.md) — Continued
  unit text can bypass the non-simple-service refusal and misclassify a service spanning installations.

- [`first-use-checkout-root-missing-durability-barriers.md`](first-use-checkout-root-missing-durability-barriers.md) —
  Newly created checkout ancestors lack parent durability barriers before allocation is committed.

- [`codex-guide-calls-trust-bypass-warning-harmless.md`](codex-guide-calls-trust-bypass-warning-harmless.md) — The Codex
  guide calls a hook-trust bypass warning harmless without disclosing its configured-hook consequence.

- [`template-editor-conceals-peer-model-and-folder.md`](template-editor-conceals-peer-model-and-folder.md) — Saving a
  cloned session opens template controls that conceal host-supplied model and folder characters.

- [`codex-transcript-error-discards-new-selection.md`](codex-transcript-error-discards-new-selection.md) — A transcript
  read error permanently discards Codex's new selection while retaining the old Resume offer; pre-existing.

## High priority: material UX degradation

- `uninstall-forgotten-success.md` — A failed uninstall can be reported as successful after another client forgets the
  host.

- `uninstall-race-first-time-service-setup.md` — Concurrent setup and uninstall can leave services pointing at a deleted
  executable.

- [`trash-listing-exceeds-control-frame.md`](trash-listing-exceeds-control-frame.md) — An oversized trash listing
  disables both listing and individual cleanup, even below the batch's entry limit.

- [`default-approval-reset-renews-retry-key.md`](default-approval-reset-renews-retry-key.md) — Default-approval
  templates repeatedly renew launch keys, defeating confirmation and unanswered retries; pre-existing.

## Other: correctness, diagnostics, cleanup, or convenience

- `template-discovery-fails-permanently-once-reply-exceeds.md` — A large valid template catalog permanently disables
  agent discovery.

- [`native-download-quit-leaves-partial-files.md`](native-download-quit-leaves-partial-files.md) — Desktop Quit leaves
  hidden partial downloads with no recovery owner.

- [`file-stat-test-compares-unresolved-temp-path.md`](file-stat-test-compares-unresolved-temp-path.md) — The new
  file-stat unit test compares a canonical result against a lexical `/tmp` path and fails on macOS.

- [`token-rotation-test-invalid-apfs-name.md`](token-rotation-test-invalid-apfs-name.md) — The new token-rotation unit
  test requires an invalid-byte directory name that APFS refuses.

- [`teststate-socket-probe-blocks-sweep.md`](teststate-socket-probe-blocks-sweep.md) — A full Unix-socket backlog can
  block the fixture cleanup probe before either cleanup deadline applies.
