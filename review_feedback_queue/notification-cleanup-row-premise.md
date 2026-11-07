# Establish the notification before testing migrated cleanup

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The new upgrade test's cleanup assertion does not prove that deleting a session removes its retained notification.
Another test independently checks notification preservation through all later migrations, so this is a narrow missing
premise in the migrated-trigger check, not absence of all notification-upgrade coverage.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/store.rs:8313-8324`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Retain original IDs g034/f4 and g035/l4 as aliases for one finding. Read the peer record and independently checked the
cited source. There is no established notification disappearance in either rebuild: the fixture inserts the row
(8107-8114), explicitly drops the notification trigger before renaming/dropping sessions (8129-8130,8178-8179), and
recreates it on the rebuilt table (8180-8182). The production migration uses the same safe ordering
(2581-2582,2629-2633), within a transaction. Neither path deletes session_notifications. Thus the concern is a
counterfactual regression the test fails to distinguish, not current fixture or migration loss.

The test claims at 8323 that 'the rebuilt table still deletes a session’s notifications with it'. It deletes
host-running first (8314), then counts zero (8316-8324). No assertion establishes that its host-running notification
exists after fixture reconstruction or immediately before DELETE. An already empty table passes that exact assertion;
the StoredSession and schema-structure comparisons do not read notification contents. Establish the exact host-running
row after reconstruction and immediately before deletion, then assert its removal.

Impact calibration: notification_resolution_migration_preserves_unresolved_history (11276-11329) plants an old warning,
restores pre-34 columns and stamps schema 27 (11278-11295), opens through the full migration ladder, asserts length 1
and exact seq/time/text/unresolved values (11297-11308), then equality after a second open and current schema
(11316-11328). This would detect a broad notification-erasing production migration. It does not supply the schema-34
cleanup test's missing before-delete premise or test that the recreated trigger acts on host-running. Revise the earlier
broad wording 'leaves one of the preservation guarantees ... unprotected' to this narrower local observability claim; do
not infer that full-ladder preservation is untested.

Suggested bucket: other, unchanged (test-evidence quality only; no production metadata-loss route established).
Confidence: confirmed source-level gap; no runtime experiment performed. proposed_drop: false; commissioner decides
final disposition. possible_cover: exact duplicate g035/l4. The independent preservation test mitigates impact; it is
not cover that repairs this test's assertion. This synthesis discovered no new defect.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g034/f4`, `g035/l4`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
