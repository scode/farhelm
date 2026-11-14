# Failed install cleanup discards the safe session-limit diagnosis

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: when installing a credential or the container's proxy settings fails because the
container host's SSH session limit is exhausted, the cleanup attempt can erase that explanation and leave only 'could
not install' in the user-visible warning. Source route: ProvisioningBackend::install_bytes delegates to
install_bytes_checked (backend.rs:464-473); SystemBackend::install_bytes_checked calls install_source (2889-2918). Its
remote branch uploads through remote_put, then runs the finishing command. Both remote_put and run_shell now classify a
nested mux refusal with exit 255 as BackendFailure::session_limit (1571-1575, 940-949), whose safe_refusal is intended
to survive sensitive-error redaction (297-324). On either failure install_source always tries remove_owned_file. That
uses another require_shell remote command (1119-1142). If the same full SSH master and failed noninteractive fallback
refuse that command too, install_source constructs BackendFailure::new from primary and cleanup strings (1332-1339),
resetting safe_refusal to None (287-292). Credential-delivery callers subsequently call redacted_reason, e.g.
credential_delivery.rs:605-608 and 649-652, so both locally constructed session-limit diagnoses are discarded in favor
of the generic credential-install warning. Proxy inheritance has the same route (environment/podman.rs:612-617).
Contract: SPEC_impl.md, One route for transport and provisioning, says 'When the fallback login fails, launch refusal or
connection unavailability names the container host's SSH session limit.' The new redacted_reason contract explicitly
promis

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g056/f2`.

### g056/f2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: when installing a credential or the container's proxy
settings fails because the container host's SSH session limit is exhausted, the cleanup attempt can erase that
explanation and leave only 'could not install' in the user-visible warning. Source route:
ProvisioningBackend::install_bytes delegates to install_bytes_checked (backend.rs:464-473);
SystemBackend::install_bytes_checked calls install_source (2889-2918). Its remote branch uploads through remote_put,
then runs the finishing command. Both remote_put and run_shell now classify a nested mux refusal with exit 255 as
BackendFailure::session_limit (1571-1575, 940-949), whose safe_refusal is intended to survive sensitive-error redaction
(297-324). On either failure install_source always tries remove_owned_file. That uses another require_shell remote
command (1119-1142). If the same full SSH master and failed noninteractive fallback refuse that command too,
install_source constructs BackendFailure::new from primary and cleanup strings (1332-1339), resetting safe_refusal to
None (287-292). Credential-delivery callers subsequently call redacted_reason, e.g. credential_delivery.rs:605-608 and
649-652, so both locally constructed session-limit diagnoses are discarded in favor of the generic credential-install
warning. Proxy inheritance has the same route (environment/podman.rs:612-617). Contract: SPEC_impl.md, One route for
transport and provisioning, says 'When the fallback login fails, launch refusal or connection unavailability names the
container host's SSH session limit.' The new redacted_reason contract explicitly promises to preserve a locally typed
session-limit diagnosis for credential and proxy warnings (backend.rs:315-318), but the intervening cleanup defeats it.
Suggested fix: preserve the trusted primary session-limit identity while combining cleanup failures, without trusting
combined peer stderr; keep the redaction behavior. Add coverage exercising an actual install_bytes failure and a failed
cleanup, rather than only require_shell/upload/probe/home/settings reads. Suggested bucket: other (diagnostic
correctness). Confidence: confirmed by source and contracts; no execution performed. proposed_drop: false.
possible_cover: none, coverage sources not consulted. Caveats: if cleanup succeeds the primary typed failure survives;
directory setup may also fail first and correctly report the session limit. This finding concerns a failure arising
after that earlier step. Could-be-highest: no established security or loss-of-work impact; the behavior over-redacts
rather than disclosing credentials. Additional endpoint source check: at 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db,
install_source still reconstructs the combined cleanup error through BackendFailure::new (backend.rs:1327-1334), and
redacted_reason still selects only by safe_refusal (307-313). Source review of capture_child confirms that run_shell
receives an ordinary completed exit and captured stderr before the new classification (1979-2020); no execution was
needed for this route. Synthesis revision, turn 2: narrowed the trigger to an accepted installation that reaches its
actual write phase. install_source first sweeps old temporaries, reads destination metadata when expected is None, may
repair/skip identical content, and removes this run temporary before entering the guarded install block
(backend.rs:1166-1188). A typed refusal from those earlier calls returns directly and can retain the intended safe
warning; this finding does not claim otherwise. A credential value replacement with different content, or a
creation-time proxy file needing installation, proceeds to remote_put. That upload can be the first failing phase, or
upload can succeed and the separate finishing checksum/guard/chmod/rename shell can be the first failing phase. Either
failure is inside the guarded block and triggers a fresh remove_owned_file request. If capacity is consumed after the
successful preflight and remains full for cleanup, both requests can fail with the typed refusal; the reconstruction at
1331-1336 loses it. Real credential callers include the Codex login and GitHub token install paths
(credential_delivery.rs:580-608 and 630-652). The real proxy call runs only on first setup before admission, after
user-manager readiness, config discovery and directory setup; already admitted containers do not reinstall proxies on
create recovery (environment/podman.rs:1186-1233; proxies:560-617). Thus this does not require proxy edits to an
admitted container or a static session-limit failure that would necessarily have stopped earlier. The required remedy is
preservation of the local typed refusal identity or rebuilding context from that type; preserving raw or combined peer
stderr in a credential/proxy warning is not safe and is not suggested. This over-redaction concern coexists with the
parent-supplied g043/c1 and g044/c2 source conclusions refuting sensitive-diagnostic exposure. Original consequence,
other bucket, confidence and proposed_drop=false remain unchanged; no runtime execution. Additional synthesis checks:
the successful install preflight does not reserve a later SSH channel. Sibling connection actors execute in separate
Tokio tasks (566 manager.rs:2023-2031); every nested upload, finishing and cleanup command independently selects the
same Supervisor master (access.rs:301-336), and the cached place-runtime return carries no channel lease
(podman_probe.rs:338-362). Source therefore permits capacity to become exhausted between successful
directory/metadata/predelete calls and upload or finishing, and remain exhausted for the cleanup request. The exact
frequency and OpenSSH scheduling were not measured or reproduced; the conclusion is the deterministic handling of that
accepted transport-failure sequence, whose mux-refusal/fallback premise is an explicit specification contract. At
endpoint 00de this turn reconfirmed new clears safe_refusal and redacted_reason chooses the fallback
(backend.rs:275-313), while install_source reconstructs the failed cleanup through new (1327-1333). No
raw-peer-diagnostic exception is needed to fix it. No new unresolved source premise or lead remains.

Returned proposal (claims-036, event 15): **retain-verified**, suggested bucket **other**.

A failed cleanup deterministically discards the trusted session-limit identity, so credential/proxy warning redaction
loses the required useful diagnosis. The accepted failure sequence is conditional source analysis, not an observed
failure or measured frequency.

Original stronger consequence: All history events keep diagnostic correctness in other. No disclosure or loss of work
was alleged; the code over-redacts. Earlier broad wording was narrowed to failures arising after successful preflight,
preserving the successful-cleanup and earlier-refusal caveats.

Unresolved premise or limitation: No missing premise for the conditional marker-loss result. Real-world frequency and
the actual OpenSSH scheduling sequence remain unmeasured; that limits a rarity-based filter proposal, not the source
conclusion.

Proposed basis: Retain the narrowly established warning defect. Preserve the original requirement to keep remote stderr
redacted and rebuild only from a trusted local refusal identity.

Existing parent disposition (event 15): verified. Parent explicitly accepted named retain-verified proposal from
claims-036 after inspecting it. A failed cleanup deterministically discards the trusted session-limit identity, so
credential/proxy warning redaction loses the required useful diagnosis. The accepted failure sequence is conditional
source analysis, not an observed failure or measured frequency. Retain the narrowly established warning defect. Preserve
the original requirement to keep remote stderr redacted and rebuild only from a trusted local refusal identity. Checked
source/results: ["At 56659839785691f76da3af5d8f51b511cff4ba8b, provisioning/backend.rs:464-473 and 2889-2918 dispatch
install_bytes through install_source. The remote guarded block uploads at 1264-1267 and finishes at
1302-1328.","backend.rs:1166-1187 performs temporary sweep, metadata/possible identical-content skip, and initial
temporary removal before that block. A refusal there returns directly; the finding requires reaching upload or finishing
after successful earlier commands.","backend.rs:940-947 and 1571-1575 classify nested SSH refusal into a typed failure.
At 1331-1338 another remove_owned_file request follows failure; its failure rebuilds through new, which sets
safe_refusal=None at 287-292. redacted_reason:319-324 therefore returns the generic fallback.","Real warning boundaries:
credential_delivery.rs:580-608,630-652 for Codex login/GitHub token, and environment/podman.rs:612-617 for proxy
settings. podman.rs:1185-1233 confines the proxy install to pre-admission setup; admitted-container recovery does not
reinstall it.","access.rs:302-324 sends independent nested commands over the place's Supervisor master. Successful
earlier commands do not reserve later command capacity. Capacity becoming exhausted before upload/finishing and
remaining exhausted for cleanup is compatible with the source; scheduling/frequency were not measured.","At inclusive
review tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, backend.rs:275-313 and 1327-1334 retain the same marker loss and
generic fallback. No raw-peer-diagnostic exception is needed to repair this."] Current coverage/results: ["Current
SPEC_impl.md:5983-6008, One route for transport and provisioning, requires the SSH-session-limit diagnosis. Named
credential catalog:5915-5919 requires fixed per-host reasons, which does not accept losing a safe local diagnosis.
SPEC.md's redaction/security rules remain compatible with retaining that local diagnosis.","TODO.md Planned is empty.
plans/queue/INDEX.md has no unlanded plans; podman-environments is already landed per REPORTS.md:21. Its session-limit
requirement is the implemented contract, not an outstanding repair plan.","BUGS.md concerns an unbounded transfer read
with escaped local helpers, not completed failed commands followed by warning over-redaction. INDEX.md and matching
queue searches found no same-root item; active TRIAGE_OUTCOMES.md has no session-limit cleanup-redaction decision. No
archive read.","FILTER.md, Rare, self-correcting glitches and imprecise diagnostics, covers the diagnostic consequence
only if the trigger is rare. Capacity changing between separate SSH commands is established, but this source-only record
does not establish a rare sub-second or otherwise narrow window. Retain rather than infer the entire filter boundary
from lack of reproduction."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved:
"All history events keep diagnostic correctness in other. No disclosure or loss of work was alleged; the code
over-redacts. Earlier broad wording was narrowed to failures arising after successful preflight, preserving the
successful-cleanup and earlier-refusal caveats."; "No missing premise for the conditional marker-loss result. Real-world
frequency and the actual OpenSSH scheduling sequence remain unmeasured; that limits a rarity-based filter proposal, not
the source conclusion.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g056 record](records/review-through-00de0b6c/groups/g056.json). Record ID: `f2`.
