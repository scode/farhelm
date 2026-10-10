# Replacement-window cleanup has the same scope-loss path

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Replacement confirmation recovery could lose evidence of surviving scoped work.

## Details

F28 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:11350` — Replacement-window cleanup has the same
scope-loss path

This replacement-window cleanup helper accepts an unconfirmed kill of the systemd process scope when portable cleanup
succeeds. After confirmation failure, window removal can mark recovery definitive and restore the prior unscoped
selection. Descendants visible only through that process group could then escape later Stop or Restart. Propagate
unconfirmed scope cleanup so recovery retains the ambiguous generation and group. This sequence was not reproduced;
fixing the other failed-relaunch helper alone leaves this path.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:11344 reaps the replacement scope using Warn at line 11350, then removes
  the replacement window.
- crates/farhelm-supervisor/src/service/core.rs:10924 converts successful cleanup after confirmation failure into a
  definitive relaunch failure.
- crates/farhelm-supervisor/src/service/sweep.rs:1409 permits unconfirmed scope cleanup under Warn.
- crates/farhelm-supervisor/src/service/core.rs:10584 and store.rs:4155 restore the prior scope selection.
- core.rs:10926-10941 maps successful cleanup_replacement_window to definitive failure. The separate helper at
  :11344-11357 uses Warn before removing the replacement window. sweep.rs:1409-1419 can return success despite an
  unconfirmed scope. The definitive recovery restores prior.scoped at core.rs:10584-10599 and store.rs:4163-4175.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- The create-rollback TODO/ledger entry covers a different operation. The partial-operation filter does not clearly
  cover later Stop/Restart bypassing a scope that still requires confirmation; uncertainty is insufficient for
  filtering.
- SPEC.md:850-855 requires cleanup before relaunch. TRIAGE_OUTCOMES.md:3448-3467 concerns prior-run cleanup; :6119-6140
  concerns failed-create rollback. Neither covers this replacement-window helper.

Caveats:

- Requires replacement confirmation failure, prior unscoped selection, an unconfirmed scope kill and a sweep-invisible
  descendant.
- No runtime reproduction.
- Later healthy Delete can recover the scope; actual work loss is not established.
- Requires confirmation failure, prior unscoped selection and a scope-only survivor after unconfirmed cleanup.
- Delete can recover once the manager is healthy.
- No demonstrated work loss or runtime reproduction.
- Preserve as a separate editable location, although both scope findings can share one implementation review.
- Requires the same unscoped-to-scoped transition and invisible survivor as F1, plus replacement-window rollback.
- No runtime reproduction or resulting work corruption was established.
- This is a distinct editable site, despite sharing recovery code with F1.
- Requires the unscoped-to-scoped transition and invisible survivor, plus confirmation failure. No runtime reproduction.
  Preserve separately from F1 because changing that helper alone leaves this helper's policy unchanged.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_03:p1:F2`, `ss_lifecycle:p1:F2`.

- `gap_supervisor_state_sec_03:p1:F2`: confidence as filed: definite, conditional on the described failure sequence;
  suggested bucket as filed: other.
- `ss_lifecycle:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
