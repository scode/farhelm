# Failed-Restart annotation test accepts errors before restoration is needed

Reviewed commit: 76ebed7af03dbd2a1ed896993d7a5b945f317e33

## TLDR

The regression can pass without proving a failed relaunch restores the user's stopped status.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/restart_under_concurrency.rs:240`.

restart_under_concurrency.rs:223-227 now proves the original exit and annotation. Lines 229-261 make the launch
directory unwritable, accept any Restart error, check that the annotation is still visible, and retry after restoring
permissions. An early refusal while the directory is unwritable could leave the annotation untouched and satisfy every
assertion. The intended boundary has a source witness: host_restart.rs:132-135 defines the claimed generation and
:265-269 attaches the launch-spec publication context to a definitive failure. SPEC_impl.md:2089-2095 requires restoring
the old outcome while leaving the new generation claimed. Check that the failure reached spec publication or that the
row generation advanced while the old outcome and annotation returned. Bucket: other, since this is a regression-test
gap. Confidence: confirmed by source. No such product regression observed. Possible coverage: host_relaunches.rs:733-752
checks a related failed-host replacement gate, but does not assert the restored stop annotation.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f3`.
