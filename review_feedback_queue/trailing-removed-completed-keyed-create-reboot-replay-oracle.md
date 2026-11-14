# Deleting the kind test also removes the completed-create reboot retry oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: removing the terminal-kind reboot test also removes its still-valid check that retrying
a successfully created session after a host reboot returns the original session, rather than creating a new one. This is
lost regression protection, not a demonstrated current duplicate-create bug. Original c1 history preserved:
terminal-kind/stream-kind fields really are retired in protocol 77, and their assertions should go; the old
proposed_drop correctly refutes the misleading-kind concern, but does not justify deleting this independent identity
oracle with them. Deleted parent create_idempotency.rs:329-382 creates an actual keyed session under boot-a, ends every
terminal host, reloads under boot-b, asserts the existing session is Interrupted, retries the same key and requires
replay.id == first.id. The retained same-boot restart fixture at reviewed :244-298 asserts ID equality and one actual
host while the host survives; it does not test key survival through reboot conversion. The retained reboot fixture at
:833-906 starts from a crashed never-launched Pending reservation and requires a launch, the opposite outcome. Store
tests are not an equivalent replacement: store.rs:13540-13645 directly checks that an interrupted row with a recorded
terminal address refuses a pending-launch takeover; it neither runs record_boot on a completed reservation nor exercises
the retry caller after conversion. store.rs:10264-10312 and :10582-10655 verify boot conversion using unkeyed rows, not
retention of a created key. core.rs:28519-28630 exercises a pending reservation that ended with ObservedExit a

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g142/c1`.

### g142/c1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 20 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: removing the terminal-kind reboot test also removes its
still-valid check that retrying a successfully created session after a host reboot returns the original session, rather
than creating a new one. This is lost regression protection, not a demonstrated current duplicate-create bug. Original
c1 history preserved: terminal-kind/stream-kind fields really are retired in protocol 77, and their assertions should
go; the old proposed_drop correctly refutes the misleading-kind concern, but does not justify deleting this independent
identity oracle with them. Deleted parent create_idempotency.rs:329-382 creates an actual keyed session under boot-a,
ends every terminal host, reloads under boot-b, asserts the existing session is Interrupted, retries the same key and
requires replay.id == first.id. The retained same-boot restart fixture at reviewed :244-298 asserts ID equality and one
actual host while the host survives; it does not test key survival through reboot conversion. The retained reboot
fixture at :833-906 starts from a crashed never-launched Pending reservation and requires a launch, the opposite
outcome. Store tests are not an equivalent replacement: store.rs:13540-13645 directly checks that an interrupted row
with a recorded terminal address refuses a pending-launch takeover; it neither runs record_boot on a completed
reservation nor exercises the retry caller after conversion. store.rs:10264-10312 and :10582-10655 verify boot
conversion using unkeyed rows, not retention of a created key. core.rs:28519-28630 exercises a pending reservation that
ended with ObservedExit and settlement, with no boot transaction. A regression that loses only Created reservations
during reboot conversion would satisfy these remaining cases but fail the deleted same-ID assertion. Source correctness
of resolve_reservation (:9884-9960) and replay_created_session (:10182-10240) does not supply the lost oracle. Contract
at SPEC_impl.md:3535-3540: 'Every keyed create gets a session-lifetime reservation: app creates, agent `create`/`clone`,
and spawn all retain the key until the created session is deleted.' The interrupted row still exists; a reboot is not
Delete. SPEC.md:3813-3818 likewise says retry returns the existing child rather than creating another and keys live as
long as that child does. Suggested fix: retain the completed-create/reboot/key-retry identity assertion while removing
only obsolete kind fields; no retired tmux fixture is needed. Suggested bucket: other (test-only regression coverage).
Confidence: confirmed source discrimination at the deletion revision, no execution. proposed_drop: false for the lost
reboot identity oracle; earlier field-retirement rationale remains applicable only to obsolete field assertions.
possible_cover: none in supplied neighbour context; no prohibited coverage sources searched. Could-be-highest: no
established shipped security or loss-of-work route; duplicate creation is a hypothetical future regression detected by
the removed test, not an observed product defect. Through-tip source search found no matching reboot replay test, and
subsequent diffs of create_idempotency.rs and boot_id_durable_outcome.rs are empty; store.rs only changes comments.
Final bounded replacement checks are complete: at pinned tip, boot-seam/key-reference searches found no completed-create
reboot replay in the remaining boot, launch-sentinel, restart-with, ticker or browser reboot fixtures. The named-create
key at restart_with_resume.rs:287 belongs to a named-start same-process test, and browser intent-key hits at
terminal-multihost.spec.ts:5095-5210 concern failed-create retargeting, not reboot. The core startup boot-file test at
deletion-revision :32223-32262 checks only the boot-id source, with no session/key. store.rs:6912-6945 confirms the
boot-conversion fixtures insert rows with no intent claim. The later core test-name/source search finds no replacement;
create_idempotency.rs and boot_id_durable_outcome.rs have no later diff through tip, and the later store diff changes
comments only. All readily checkable premises for this bounded coverage finding are resolved; no open lead remains for
c1. No tests, builds or reproductions.

Returned proposal (claims-090, event 20): **retain-verified**, suggested bucket **other**.

Retiring terminal-kind fields justifies removing their assertions. It does not justify removing the
completed-create/reboot/same-key identity oracle in the same test. The checked retained tests exercise same-boot restart
or never-launched reboot reservations instead.

Original stronger consequence: Original c1 considered misleading terminal-kind reporting and duplicate creates after
restart, and initially proposed a field-retirement dismissal. Later source checks split that: obsolete fields are
refuted, but the distinct lost completed-create/reboot identity oracle is retained. Duplicate processes are a
hypothetical regression the deleted test could detect, not an established shipped failure or user-work loss.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the narrow test-coverage finding. Preserve the earlier field-contract refutation only for removed
representation assertions; do not use it to drop the independent reboot/key sequence.

Existing parent disposition (event 20): verified. Parent explicitly accepted named retain-verified proposal from
claims-090 after inspecting it. Retiring terminal-kind fields justifies removing their assertions. It does not justify
removing the completed-create/reboot/same-key identity oracle in the same test. The checked retained tests exercise
same-boot restart or never-launched reboot reservations instead. Retain the narrow test-coverage finding. Preserve the
earlier field-contract refutation only for removed representation assertions; do not use it to drop the independent
reboot/key sequence. Checked source/results: ["f560058da19e02716e6f943883a9508b447aff4e diff of create_idempotency.rs
deletes a_replay_after_a_reboot_reports_no_terminal_kind, including real completed keyed create, terminal-host
termination, boot-a to boot-b handoff, Interrupted premise and replay.id == first.id. Field assertions can be removed
independently of that sequence.","f560058d:create_idempotency.rs:244-300 retains same-boot restart identity and
one-agent assertions. :833-906 injects AfterRecord failure, retains a Pending reservation through reboot and requires a
launch; this is the opposite initial outcome from the deleted completed-create
fixture.","f560058d:crates/farhelm-supervisor/src/store.rs:13539-13645 checks pending takeover against directly forced
Interrupted rows, including one with an address; it does not run reboot conversion on a completed reservation or retry
the real caller afterward.","f560058d:SPEC_impl.md:3535-3540 requires session-lifetime create reservations. Current
SPEC.md:3805-3811 keeps retry identity until child deletion. A surviving Interrupted row was not deleted.","git diff
from f560058d through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty for create_idempotency.rs and
boot_id_durable_outcome.rs. Pinned-tip boot-id/key search in those files finds the retained Pending case and unkeyed
boot fixtures, not the removed completed-create retry. No runtime duplicate creation is claimed."] Current
coverage/results: ["Both specifications retain key/session lifetime; SPEC_impl.md Grid streams accepts retirement of
terminal-kind fields only. It does not accept loss of the independent reboot identity test.","TODO.md Planned empty.
Targeted plans/queue searches for keyed/idempotency/reboot/replay found provider restart and terminal-kind removal work,
no completed-create/reboot replay test replacement.","BUGS.md has no such accepted bug. Full INDEX.md and matching
queue-file searches found no same test deletion or reservation/reboot oracle. Existing
reboot-concurrency-early-release.md concerns permit lifetime during teardown, a different cause and
consequence.","Active TRIAGE_OUTCOMES.md names unrelated GUI create cleanup and test witness findings; no exact
completed-create/reboot replay disposition. FILTER.md does not cover deletion of this applicable identity oracle."]
Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Original c1 considered
misleading terminal-kind reporting and duplicate creates after restart, and initially proposed a field-retirement
dismissal. Later source checks split that: obsolete fields are refuted, but the distinct lost completed-create/reboot
identity oracle is retained. Duplicate processes are a hypothetical regression the deleted test could detect, not an
established shipped failure or user-work loss."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g142 record](records/review-through-00de0b6c/groups/g142.json). Record ID: `c1`.
