# Source-verified conditional SSH bootstrap cleanup gap

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Source-verified conditional SSH bootstrap cleanup gap

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g070/l1`.

### g070/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 19 (finding, open):

Confidence reconciliation, turn 5: confirmed conditional source route for rootless Podman 4.9.3 with its pinned storage
1.51.0/common 0.57.4, systemd 255 and a non-PTY OpenSSH 9.6 connection to the explicitly selected localhost place. This
is a supported implementation/configuration, not a claim those versions or a filesystem stall were observed on this
host. The exact failure condition is that the remote first-builder command is still before libpod registration when the
real 120-second capture deadline expires and remains there through the subsequent normal inventory reads. The checked
pre-registration filesystem call gives an actual operation boundary where this condition can hold; the test/provider
timeout and error-reply-to-assertion-panic path are executable source branches. Under that condition the storage-only
builder is invisible to the exact non-external scans, its Ubuntu source is deliberately exempt from image cleanup, and
non-PTY SSH channel teardown does not remotely kill it. The resulting false cleanup success/late publication follows
from the checked branches. No further platform mechanism is asserted or needed for this conditional handling claim.
Actual occurrence, delay duration, frequency and this machine's installed versions remain unobserved
incidence/configuration facts, not an uninvestigated missing source premise. This does not claim that a generic syscall
necessarily stalls, that every storage delay leaves inventories usable, or that the ordinary local/derived-image/commit
branches share this route. Earlier likely/UAI/confirmed labels and broader routes remain preserved below as history,
superseded in scope by this reconciliation. No reproduction or runtime operation; no parent drop or lowering accepted.

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. Original consequence retained and narrowed: fallback can
report successful cleanup while an owned builder from the real SSH acceptance remains in Podman storage and can later
become a running container. This consumes only this test's resources; no security or user-work-loss route is
established. Suggested bucket: other. Confidence revised to likely, naming the external occurrence premise: a
first-bootstrap remote Podman create stalls in a pre-libpod-registration filesystem operation past the 120-second
command deadline and the fallback scans, while unrelated inventory reads remain usable. The conditional source route is
now traced; no such storage stall or leaked process was observed. proposed_drop: false; possible_cover: none;
could-be-highest: no.

Useful storage checks refute the earlier broad implication that any missed derived-image create can pass fallback:
Podman 4.9.3 pins containers/storage v1.51.0 and common v0.57.4 (go.mod:13,20). store.go:1744-1758 locks layers/images
while CreateContainer resolves its source; :1871-1889 records the storage container and its ImageID before releasing
those locks. DeleteImage uses writeToAllStores (:1349-1372), then :2551-2560 refuses an image referenced by ANY storage
container, including one not yet registered in libpod. Thus a derived-image create either loses its source image before
storage creation, or protects that image and makes the fallback's final image-empty check fail. Podman's
container_commit.go:42-49 holds the container lock across Commit at :169; ordinary removeContainer takes that lock, so
the claimed late-commit shortcut is not established. These narrower refutations preserve the original consequence but
remove those proposed routes.

The remaining source-conditional route uses the first bootstrap builder, whose source is deliberately NOT swept:
podman/build.rs:41-45 runs it from cached UBUNTU, before any test-owned image exists. Podman runtime_ctr.go:442-455
completes storage setup, then calls os.MkdirAll for the container secrets directory before libpod state.AddContainer at
:566-578. This filesystem boundary is outside CreateContainer's image/layer locks. Podman storage.go:123-129 creates the
storage container; common inventory ps.GetContainerLists uses runtime.GetContainers at pkg/ps/ps.go:64 and adds
storage-only external containers only when options.External at :89. The fixture's owned_ids command does not pass
--external. Therefore during a successful-but-stalled pre-registration mkdir, normal container ls cannot see the
builder. The Ubuntu base is unlabelled/user-owned for this test and intentionally retained, so no test-owned image
removal supplies the otherwise protective image-use check.

Failure/child-lifetime sequence: this fixture's SSH route is ordinary non-PTY localhost SSH. SystemPlaceCommands spawns
an isolated local SSH process group, and capture_child on deadline kills/reaps that group
(backend.rs:1979-2019,2268-2278); it does not remotely signal the Podman PID. OpenSSH V_9_6_P1 session.c:2483-2523
explicitly handles a channel dying before its child: absent a PTY it closes/detaches channel state without killing that
child. systemd v255 run.c:1535-1721 moves the command through the transient scope job then execs it; that scope is not
the local SSH client's group. Podman detached run creates/starts first (ABI containers.go:1129-1165); cmd
containers/run.go:214-225 prints its ID only after the engine returns. A closed output pipe is therefore not an implicit
pre-create cancellation. This is an already-running external process blocked on named filesystem work, not a Rust future
resuming after helm death or an arbitrary scheduler pause.

After the remote command deadline the production prepare path attempts reconcile_helpers, whose normal libpod inspect
cannot resolve this storage-only builder; its persisted recent pending name is retained (:630-675). There are no retired
images yet (:41-45), so prepare_images returns the error (:267-289). environment_session_steps returns it
(tensorlake_add.rs:521-539); finish_session_launch emits an error reply (:1028-1057), and the test's reply[error]
assertion at podman_environments.rs:826-830 panics. Its catch boundary then stops/reaps the helm and runs fallback
(:1276-1293). Fallback scans only visible recipe labels (:1217-1251), never the pending name/storage-only inventory. Its
empty container/image checks can therefore succeed before the stalled remote operation completes; once it does, libpod
registration/start can continue. This route uses an explicit handled command timeout followed by a reachable test
assertion, not a conjectured 30-minute timeout during the early first run.

External primary sources checked: https://raw.githubusercontent.com/containers/podman/v4.9.3/go.mod ;
https://raw.githubusercontent.com/containers/storage/v1.51.0/store.go ;
https://raw.githubusercontent.com/containers/common/v0.57.4/libimage/image.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/libpod/storage.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/libpod/runtime_ctr.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/libpod/container_commit.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/pkg/ps/ps.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/pkg/domain/infra/abi/containers.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/cmd/podman/containers/run.go ;
https://raw.githubusercontent.com/systemd/systemd/v255/src/run/run.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/session.c . Earlier web-rendered upstream line
numbers stripped blank lines; the new citations here use raw numbered source.

No uninvestigated source branch remains for this stated conditional route. Unmeasured premises are occurrence/duration
of the specified filesystem stall and actual installed engine/SSH versions, storage backend and configured wrappers; the
test accepts a supported range and does not pin those host facts. Reading the same functions again cannot establish that
an actual run experiences the stall. A version/config manifest or authorized observation could establish occurrence, but
no reproduction or process/live-install operation is requested or performed. This is likely conditional source analysis,
not a confirmed observed leak. The early confirmed wording and prior UAI details remain in append-only history; no
original consequence or proposed-drop basis is erased.

Final lookup/cleanup composition checks this turn: v4.9.3 ABI containers.go:161-174 checks libpod LookupContainer for
exists and consults storage-only containers only with options.External; :486-537 container inspect also uses
LookupContainer. Thus the provider inspect gate at app podman.rs:226-229 really can return absent during the named
storage-only interval. ps.go:28-106 confirms normal inventory excludes external storage unless explicitly requested. On
initial preparation failure the session binding still has no session-container ID/pending-create marker; app
podman.rs:1298-1301 therefore certifies that separate session resource never created, while the builder pending name
remains in recipe state. finish_session_launch:1028-1057 can return its failure without settling this ambiguous builder.
These checks close the stated application composition rather than relying on normal recipe cleanup. No new missing
source premise, runtime observation or broader local-container/derived-image claim was added.

Returned proposal (claims-044, event 19): **retain-verified**, suggested bucket **other**.

The narrowed SSH bootstrap route is source-established under its explicit pre-registration delay condition. Fallback
scans only registered objects and can finish while an owned remote builder remains storage-only; its user-owned Ubuntu
source is outside the image sweep. This is private test-resource leakage, not an observed incident or a production
data-loss claim.

Original stronger consequence: Line 5 broadly called late publication by a surviving provider create confirmed; lines
10/14 revised confidence to unverified after investigating missing external composition. Lines 17-19 preserve owned
containers/storage left after apparent cleanup, but narrow proof to first SSH bootstrap. Earlier
local/derived-image/commit routes are not re-established: the history reports storage image-reference locks and
commit/container locks defeating their easy late-publication argument. Keep those refutations and the prior confidence
reversals; do not summarize the latest as proving all old routes.

Unresolved premise or limitation: No missing source premise for the latest conditional route. Occurrence requires the
remote first builder to remain pre-registration past 120 seconds and through otherwise usable fallback scans. Actual
duration, incidence, installed versions and wrappers remain unobserved. No claim that every filesystem stall permits
usable inventories.

Proposed basis: Retain the latest source-verified conditional SSH-only handling gap and every external-condition caveat.
All claimed retained resources are run-owned maintainer fixtures; no catastrophic tooling or user-owned-work consequence
was established.

Existing parent disposition (event 19): verified. Parent explicitly accepted named retain-verified proposal from
claims-044 after inspecting it. The narrowed SSH bootstrap route is source-established under its explicit
pre-registration delay condition. Fallback scans only registered objects and can finish while an owned remote builder
remains storage-only; its user-owned Ubuntu source is outside the image sweep. This is private test-resource leakage,
not an observed incident or a production data-loss claim. Retain the latest source-verified conditional SSH-only
handling gap and every external-condition caveat. All claimed retained resources are run-owned maintainer fixtures; no
catastrophic tooling or user-owned-work consequence was established. Checked source/results:
["04d85f0c98a9b635709d00b056719a0baa7fd3fc introduces crates/farhelm/tests/e2e/podman_environments.rs (1,330 added
lines). owned_ids:1160-1194 uses ordinary container ls without --external; cleanup:1198-1251 stops only the helm and
scans current labels. Assertion panic/timeout reaches cleanup at :1276-1293. Pinned tip retains this logic at
:1491-1588.","Same revision:environment/podman/build.rs:41-45 starts the first builder from user-owned UBUNTU; :554-565
persists its pending name before run; :267-289 attempts helper cleanup on failure; :630-675 retains recent absent names.
This state is not consulted by fallback.","Same revision:environment/podman.rs:295-320 uses an independent transient
scope and a 120-second command deadline. podman_probe.rs:178-205 isolates the local SSH client;
provisioning/backend.rs:1979-2019,2268-2278 kills/reaps that local group on timeout, without a remote Podman completion
receipt.","Same revision:provisioning/tensorlake_add.rs:521-539 propagates failed preparation; :1028-1057 reports
failure after session-resource cleanup. environment/podman.rs:1298-1301 considers the separate session container never
created when its handle has neither ID nor pending marker. Test :826-830 panics on that launch error; this is a
reachable failure boundary before the scenario's long timeout.","Podman v4.9.3 libpod/runtime_ctr.go:442-455 does
setupStorage then secrets-directory MkdirAll before :566-578 adds libpod state. libpod/storage.go:123-129 creates
storage; pkg/ps/ps.go:59-89 inventories libpod and adds storage-only containers only with External. These branches
establish invisibility during the specified delay. URLs:
https://raw.githubusercontent.com/containers/podman/v4.9.3/libpod/runtime_ctr.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/libpod/storage.go ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/pkg/ps/ps.go .","OpenSSH V_9_6_P1 session.c:2483-2523 handles
a channel closing before its child without a child kill for the non-PTY case. Podman ABI containers.go:1129-1165
completes create and detached Start before returning, so the output-pipe close is not itself a pre-create cancellation.
URLs: https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/session.c ;
https://raw.githubusercontent.com/containers/podman/v4.9.3/pkg/domain/infra/abi/containers.go ."] Current
coverage/results: ["SPEC.md Container sessions requires Podman >=4.9.3 and retains user-owned base images. SPEC_impl.md
Shared recipes and prepared artifacts requires ordinary recipe deletion to wait for active commands, and Podman places
describes independent scopes. These explain the intended product safeguards but do not accept the test fallback's
missing completion boundary.","Current TODO.md Planned empty. plans/queue searched for bootstrap/storage-only/fallback
cleanup and provider tests. podman-environments is landed per plans/REPORTS.md:21; its normal product
cleanup/verification criteria do not prescribe a pending-command-safe fallback. No unlanded repair found.","BUGS.md
concerns a different local SSH output-holder transfer hang. Active TRIAGE_OUTCOMES.md searched for Podman, bootstrap,
storage-only and fallback cleanup; no matching decision. Archive not read.","INDEX.md and matching queue coverage
searched. podman-storage-selection-escapes-lifecycle-lookup concerns engine configuration selection;
recipe-cleanup-navigation concerns browser navigation; route-retirement-failure-skips-owned-session-cleanup concerns
browser teardown prerequisites. None is an exact alias or acceptance of the SSH bootstrap fallback route.","FILTER.md
full boundary does not match. The trigger is a remote provider operation before registration, not a hung user manager;
successful cleanup with retained work is excluded from the rare safe-failure/diagnostic filter."] Exact cited cover: []
Original stronger consequence and missing premise remain preserved: "Line 5 broadly called late publication by a
surviving provider create confirmed; lines 10/14 revised confidence to unverified after investigating missing external
composition. Lines 17-19 preserve owned containers/storage left after apparent cleanup, but narrow proof to first SSH
bootstrap. Earlier local/derived-image/commit routes are not re-established: the history reports storage image-reference
locks and commit/container locks defeating their easy late-publication argument. Keep those refutations and the prior
confidence reversals; do not summarize the latest as proving all old routes."; "No missing source premise for the latest
conditional route. Occurrence requires the remote first builder to remain pre-registration past 120 seconds and through
otherwise usable fallback scans. Actual duration, incidence, installed versions and wrappers remain unobserved. No claim
that every filesystem stall permits usable inventories.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g070 record](records/review-through-00de0b6c/groups/g070.json). Record ID: `l1`.
