# Automatic Update can mutate an unadopted skewed installation

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Automatic Update can change a replacement installation at the saved SSH destination before discovering its new identity
and refusing attachment.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

A recreated older-protocol installation can remain Skew with no client identity, while the registered entry retains its
old accepted identity. Enabling automatic updates in that stable state supplies a shipped producer: the no-client
incarnation tuple remains stable through planning/preflight, which can change payloads, modes and the ordinary unit
before full attachment observes the identity mismatch. It does not recursively delete state or checkout folders.
SPEC_impl.md:5459–5474 explicitly accepts identity-unverified Update and its cost; SPEC.md makes automatic opt-in
authorize the same Update, but the distinct-identity/adoption wording leaves the broader automatic-authority
interpretation ambiguous. Source confirms the bounded conditional mutation; no independent trust-boundary or
user-work-loss route is established.

Source locations: `crates/farhelm-helm/src/provisioning/service.rs:1331-1336`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Original owner/ID g139/f3; independent exclusion audit a038/f11. Reviewed full commit
d6645f771bee8ed03a66486ef8755834937d759d. TLDR: automatic protocol Update can change a replacement installation at the
registered SSH destination before its identity is adopted, then fail attachment when that identity becomes usable.
Conditional mutation is source-confirmed; no runtime occurrence is required. Suggested bucket OTHER; no stronger
user-work/security harm is established. proposed_drop=false: retain while automatic-authority interpretation is
ambiguous. Newly precise possible_cover: SPEC_impl.md:5459-5474 explicitly accepts identity-unverified UPDATE and names
its cost, and SPEC.md:118-120,144-153 makes opt-in authorize the same Update. This strongly covers the bounded
mechanism, not arbitrary wrong-installation harm; general same-entry acceptance is not a substitute. SPEC.md:228-249
describes wiped/reinstalled hosts as distinct identities requiring adoption, whereas :252-262 concerns intentional
retarget/adoption. Complete conditional history: a normal panel-managed SSH installation D was recorded as A; while
stopped its state is wiped/recreated, and the existing unedited unit starts the older binary at the same paths, minting
B. This is the product's explicit wiped-install condition, with one helm and unchanged destination, paths and entry
UUID. Exclusive-base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c core.rs:5540,5629-5651 creates/opens state and mints
identity; store.rs:5399-5435 uses a new random UUID. Current equivalents: core.rs:5700-5720,5789-5811 and
store.rs:5419-5460. Base protocol=64, current=67; both supervisor schema=31. With an old build not newer than the helm,
normal supported Linux/systemd/SSH, writable default paths and a normal unmarked panel unit,
version/schema/architecture/ownership refusals do not block the case. The protocol-67 helm rejects the lower-protocol
hello before identity acceptance: proto io.rs:542-600 parses Hello but VersionSkew retains protocol/build/schema, not
identity; manager.rs:3766-3805 returns Skew before :3910-3936 can compare B to A, and :3362-3385 publishes skew with
client=None through :3230-3238. Enable automatic updates for its first attempt while B is already stably skewed; this
shipped producer needs no sub-second race. automatic.rs:77-90,103-144 plans and starts it using the entry
UUID/incarnation. manager.rs:1356-1364 makes same_connection(None,None)=true, so :4666-4669 preserves the tuple through
no-client reprobes. Even a new tuple after the old full connection disappeared proves no continuity with A.
service.rs:1118-1205 plans through unchanged D while carrying A unverified, :1318-1336 checks policy and tuple, and
:1803-1819 holds the provisioning lock during :1998-2093's unchanged-registration/trust/skew preflight.
store.rs:6050-6081 keeps A and the existing UUID when discovered_identity=None. plan.rs:565-719,942-971 freezes the
replacement destination's actual program, state and unit paths; service.rs:2256-2279,2425-2488 executes the frozen
actions. backend.rs:2176-2251,3167-3269 accepts that ordinary panel unit; :2282-2304 changes private directory modes,
:1032-1163 uploads and mv-replaces verified payloads, :877-1026,2357-2384 writes the unit checking only the separate
setup marker, and :2387-2418,2459-2471 reloads/enables/restarts it. Normal affected paths are the remote resolved
~/.local/lib/farhelm/farhelm, optional private tmux, registered ~/.local/state/farhelm and systemd user-unit
directory/farhelm-supervisor.service; linger is another planned mutation. Update does not recursively delete B's state
or working folders. Only post-update full-protocol attach observes B versus A: manager.rs:3910-3936 yields Mismatch and
service.rs:2524-2578 with attach_refusal :256-261 fails the run. That prevents silent adoption but does not undo
preceding writes. Difference from original g139/f5: ordinary full client change/disconnect changes incarnation, retarget
explicitly changes it, registration edits share the execution lock, and full identity-bearing preflight refuses mismatch
before actions. A no-client skew tuple identifies the same registry actor, not its installation. Missing for highest is
a compatible ordinary source history where these bounded Update actions lose user-owned work, cross a concrete
independent trust boundary, or exceed automatic opt-in authority beyond the explicit skew relaxation. Source establishes
replacement mutation and post-update refusal, not that stronger consequence. Exact policy interpretation or a complete
supported harmful source sequence would settle the remainder; runtime occurrence is neither required nor requested.
Existing g139 gated empty discovery provides no stronger exclusion approval. No new duplicate ID is created. A first
attempt to record this revision failed because the private event JSON contained raw newlines; no finding event was
appended by that failed invocation. This valid replacement preserves that failure note. No tests, experiments or live
actions occurred; separate discovery remains pending.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a038/f11`, `g139/f3`, `g107/f5`,
`a029/a029-v5`. These are original reviewer identities for this concern, not additional bugs. Full revisions,
withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was executed.
