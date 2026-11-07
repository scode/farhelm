# Fake filesystem cancellation outlives test transaction

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

A cancelled asynchronous fake-filesystem operation can continue into a later test transaction and change its state.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/e2e.rs:428-434 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g052/l5, event 25):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: browser tests that delete a recipe during its snapshot
build can race the Tensorlake fake's own state-file updates, producing erroneous cleanup failures or corrupting the
fake's recorded resource state. The issue is in maintainer test infrastructure, not live Tensorlake billing. Mechanism:
change_model at e2e.rs:428-434 holds an async mutex while awaiting tokio::fs::write on shared state.json.next and rename
onto state.json. create_snapshot/get_snapshot call change_model; build.rs:1123-1136 wraps both calls in remote(), whose
cancellation select at :713-724 drops the fake API future. clean_for_delete at :1265-1277 signals this cancellation and
awaits only the build task, then resumes fake resource mutation for cleanup. Dropping the API future releases
state_write even if filesystem work has already been submitted. Exact locked Tokio 1.53.2 primary source was read from
the existing registry cache: fs/write.rs write_spawn_blocking owns path/buffer and calls asyncify(std::fs::write);
fs/rename.rs rename_blocking calls asyncify(std::fs::rename); fs/mod.rs:312-323 awaits a spawn_blocking JoinHandle. Such
work continues after its JoinHandle is dropped, and task/blocking.rs:106-120 explicitly says started blocking tasks
cannot be aborted. Therefore cancelled operation A may still write/rename the very same .next pathname while later
operation B acquires the supposedly exclusive mutex and reuses it. A delayed rename can consume B's temporary file,
making B's rename fail with ENOENT; overlapping write/truncate operations can publish mixed or stale state. This defeats
both the serialization comment and the atomic-observer model promised by SPEC_impl.md Tensorlake account's fake-state
description. Suggested bucket other, confidence confirmed source-level cancellation/ownership race for Tokio's normal
blocking filesystem implementation; no runtime reproduction performed. Need complete temporal check at pinned tip and
verify normal project feature configuration before final disposition. suggested fix: make the entire read-modify-write
transaction independently owned and uncancellable with its mutex held through actual filesystem completion, or run it
under a synchronous mutex inside one blocking task; unique temp paths alone do not prevent a cancelled stale transaction
from publishing later. proposed_drop false; possible_cover unknown, parent handles coverage. This is a distinct root
mechanism from l1's dropped SSH process supervisor, known termination deadline findings or project-key changes: here the
API fake's filesystem task outlives its released serialization lock. No real cloud calls or process experiments were
run. Final checks: original and pinned-tip Cargo.toml:228-239 enable ordinary Tokio fs/rt-multi-thread and do not enable
io-uring; no tokio_unstable/io-uring entry was found in the scoped Cargo/.cargo query, and .cargo/config.toml does not
exist at the original revision. Exact 1.53.2 fallback fs path therefore applies to the normal project build; unusual
externally enabled unstable feature configurations are not required for this finding. At pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b, e2e.rs:464-481 retains the same mutex/shared-next-file transaction,
get_snapshot:597-604 still mutates through it, and build.rs:804-814,1214-1228 still cancels by dropping the API future.
No later repair. Confidence kind verified: supported cancellation path plus non-cancelling background filesystem
ownership establishes the race without a runtime experiment. Exact reproduction frequency under a particular OS
scheduler has not been measured and is not a missing premise for the ownership defect. The race can also arise when
multiple recipes share this single fake and a new model mutation follows a cancelled one. Suggested verification later:
pause or instrument the first filesystem transaction, cancel the API future, start a second mutation, and assert
transaction serialization plus intact model JSON without changing process environment. Scope remains browser fake only;
no real Tensorlake project-ID/status behavior is assumed. Discovery pass 2 novel verified ID l5; no novel
investigated-unverified finding so far.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g052/l5`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
