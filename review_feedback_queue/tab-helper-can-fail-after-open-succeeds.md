# A delayed tab helper can fail after Open succeeds

Reviewed commit: 8b38edd6416271e2cf7b82bc3b2d786ea7b720f2

## TLDR

A tab can pass its short initial liveness check before its helper fails to enter the folder, then disappear without the
promised directory error. No wrong-directory command execution or work loss is established.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `plans/reports/launch-enters-working-dir.report.md:115-118 (deleted side)`. Locations and revisions in
the assessment below identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 8b38edd6416271e2cf7b82bc3b2d786ea7b720f2. Historical product provenance: deleted-side
plans/reports/launch-enters-working-dir.report.md:115-118 explicitly records this remaining failure; deleting the report
did not introduce the executable mechanism. TLDR: a requested terminal tab can be reported opened before its helper
enters the chosen folder; a later folder-entry failure can then lose the required error. Source at this commit:
service/core.rs:13417-13426 launches the full Farhelm tab helper, 13551-13607 accepts any live pane and returns
Ok(TabInfo), and 14031-14059 bounds this to pane liveness rather than helper readiness. service/terminals.rs:24-41 sets
the 250 ms window. launch.rs:1185-1201 enters the folder only in the helper and returns on error; main.rs:1499-1501
propagates that error through the helper CLI, without an acknowledgement to the supervisor. The conditional schedule is
that the helper remains live before its directory check until after this window, then chdir fails. The requested shell
never starts elsewhere. SPEC.md:1381-1390 and 3203-3209 require a clear folder-entry error; SPEC_impl.md:2694-2698
assumes the early-death refusal carries it. SPEC.md:1409-1413 silently reaps post-open exits, which is why the error is
not retained. Protocol lib.rs:3332-3353 calls the success an open plain shell, whereas the observed live process can
still be its helper. The same acceptance/check code and helper remain at pinned tip
463cac1528b5c5e259404975380d321a3fff0412, same cited core/launch line ranges. Suggested bucket: other; no owned-work
loss, wrong-directory execution or security route shown. Confidence: confirmed conditional source mechanism;
scheduling/loader-delay frequency is unmeasured, no runtime evidence was authenticated or collected. possible_cover: no
exact match found in pinned SPEC.md, SPEC_impl.md, empty TODO Planned, BUGS.md, queue INDEX.md or matching ledger
searches. TRIAGE_OUTCOMES.md:15779-15790 requires clear failure and records the original repair, not acceptance of this
remainder. FILTER rare-diagnostic rule might cover the missing error alone, but excludes success reported for an
operation that failed. The normal post-open-shell-exit rule is narrower than a helper that never starts the shell due to
failed directory entry. proposed_drop: none. This corrects the initial pass's attribution-only exclusion of the same
historical caveat.

Provenance: `g156/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

Container tab preparation reaches the same helper-liveness versus actual-shell-readiness boundary. The reviewed
token-to-FIFO producer is deliberate external file reshaping, explicitly accepted by SPEC.md:3624–3627,3676–3689. That
exact producer is not claimed as a new accidental defect, while coverage of the separate false-success shell contract
remains ambiguous. No ordinary regular-token Podman delay/failure producer was established after source checks. Preserve
the original g117 withdrawal and this qualified extension question; do not infer a generic security or work-loss route
or discard the existing directory-entry cause.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `a004/x6`:
  `crates/farhelm-supervisor/src/service/core.rs:14081-14137; crates/farhelm-supervisor/src/container.rs:112-127; SPEC.md:3624-3627,3676-3689`.
- `a021/x-g117-f4`: `crates/farhelm-supervisor/src/service/core.rs:14055-14137`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
