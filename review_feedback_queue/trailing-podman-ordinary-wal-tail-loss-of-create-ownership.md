# Integrated container ownership keeps the accepted WAL power-loss limit

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Integrated container ownership keeps the accepted WAL power-loss limit

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g068/l2`.

### g068/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (finding, open):

# Integrated container ownership keeps the accepted WAL power-loss limit

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b

## TLDR

An OS crash or power loss on the helm machine can lose recently committed container ownership or create facts even
though a remote container survives. This can require explicit leftover cleanup or manual recovery of running work. This
is a real integrated creation route, not the foundation revision's missing-caller case, but the specification explicitly
accepts ordinary WAL tail loss.

## Details

register_container uses self.conn.call and commits registration at store/podman.rs:373-394. set_podman_prepared_create
atomically writes login, selected install facts and pending send with self.conn.call at 268-278.
environment/podman.rs:1103-1125 awaits that write before sending run. Canonical ID observation uses the same ordinary
call at store/podman.rs:422-432; installation admission does too at 315-329. Db::call only locks and runs work
(farhelm-supervisor/src/db.rs:230-245); it does not upgrade durability. configure_durability sets NORMAL when actual
mode is WAL (db.rs:94-117). The separate FULL/EXTRA helper is invoked only explicitly (db.rs:247-266). The integrated
provisioning caller registers before preparation (provisioning/tensorlake_add.rs:388-408), creates before reading saved
facts/admitting paths (555-594), then delivers credentials (597-605), attaches, and creates the session in
finish_session_launch (981-1013). No mandatory full-sync barrier is supplied by these ownership APIs before external
create. A remote place can remain alive when only the helm machine loses power. This establishes potential loss of the
registry/marker association, not database corruption or automatic destruction of the remote files.

Recovery is conservative: decode_binding requires complete login for created/admitted rows (store/podman.rs:148-165);
create adopts a name only with recorded canonical or pending identity (environment/podman.rs:1071-1093), otherwise it
refuses the collision. destroy returns never-created when both ID and pending marker are absent (1298-1302), so loss of
those records cannot by itself authorize deleting the surviving resource. Normal initial-launch failure explicitly
cleans only its new environment (provisioning/tensorlake_add.rs:1028-1044); a power failure does not resume this task at
startup. Label-based leftovers are listed at usable places and places referenced by rows or recipe artifacts
(environment/podman.rs:771-835), with rowless explicit deletion ownership-checked at 725-765. Thus tail loss can strand
work from its normal UI association and require manual recovery, but available source does not establish irreversible
loss of the files/processes themselves. A helm process crash alone preserves committed ownership under the stated
contract.

Exact specification acceptance, SPEC_impl.md Helm internals:5668-5672: 'Its ordinary store opens share the supervisor's
journal setup: owning opens attempt WAL before migrations, warning and continuing in the current mode if conversion
fails or does not return `wal`. Non-owning opens never convert; every ordinary read-write connection selects
`synchronous=NORMAL` only after reading an actual WAL mode and otherwise retains FULL. This accepts losing the last
commits on an OS crash or power loss without weakening database consistency; process crashes preserve commits.' Same
contract at 6683-6685 explicitly says: 'A legacy row or a power loss that lost both records cannot recover by name:
remove that host entry, then handle the leftover explicitly.' Snapshot-build ownership at 6840-6841 says: 'Ordinary
write durability is retained: a legacy row or power loss that loses both the ID and pending marker requires explicit
cleanup.' The Podman integration continues ordinary store durability rather than introducing a listed trust-revocation
exception.

Suggested bucket: high for the source-established lost normal access/cleanup association if reported; could-be-highest:
yes, the original potential loss of user work/processes is preserved for independent checking, but no shipped
irreversible-loss or silent-destructive-recovery route was established. Confidence: confirmed source and contract
analysis of the write policy/caller; no execution or reproduction and no claim that a particular crash was observed to
lose a commit. proposed_drop: exact explicit specification acceptance of ordinary WAL power-loss tail loss and explicit
cleanup above, NOT missing integrated caller. Retain as a finding with proposed_drop for the independent check because
the original concern could be highest. possible_cover: g064/c3 examines the foundation/endpoint scope; this record
supplies the integrated assigned-store caller and recovery analysis. No readily checkable missing premise remains for
this conclusion.

Returned proposal (claims-043, event 6): **covered**, suggested bucket **high**.

The integrated route uses ordinary durability, and ordinary helm WAL tail loss on OS crash or power loss is explicitly
accepted. Source supports lost normal association and manual recovery, not automatic destruction of surviving work. This
proposed coverage and consequence limit need independent checking.

Original stronger consequence: Power loss could erase ownership facts while remote work/processes survive, potentially
leaving user work unaccounted for or inaccessible. Later wording narrows established consequence to lost normal
association/manual recovery; it does not erase the original possible work-loss concern.

Unresolved premise or limitation: Irreversible work/process loss or a silent destructive recovery route was not
established. The accepted loss of the normal UI/cleanup association is established conditionally on lost WAL tail
commits.

Proposed basis: Exact ordinary-durability specification acceptance, applied to the integrated caller. Do not use the
earlier foundation revision's absent production caller as the drop basis. Independent check must also assess the
proposed limit to association loss rather than irreversible loss.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: ordinary-WAL-power-loss-ownership-policy-cover-with-original-consequence-limit-gate. Full043
integrated caller actually awaits ordinary pending/identity writes before externalrun; globalWAL/NORMAL accepts OS/power
tail loss, not processcrash loss, and no FULL trust exception covers container ownership. No-ID/no-pending refuses name
adoption, leftovers require explicit ownership/deletion and may need manual recovery. Proposed same064c3/065l3 policy
cover, not earlier foundation no-caller dismissal. Preserve loss of normal association and original possible
irreversible work/process harm; exact accepted scope and proposed high consequence limit need fresh judgment.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g068 record](records/review-through-00de0b6c/groups/g068.json). Record ID: `l2`.
