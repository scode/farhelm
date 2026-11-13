# Historical installation policy keeps a newer-tmux warning promise after its desktop emitter retires

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Historical installation policy keeps a newer-tmux warning promise after its desktop emitter retires

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g160/l2`.

### g160/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 24 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333; warning path retires earlier within the range at dbfc69751215dbf15e1d31d5bed73c3754c4ebc4; Mac prerequisite/preflight is removed later at 17a3fe858c5511a24b7403af15dd41c93bf69fd7. Original TLDR preserved: A Mac user starting Farhelm with a newer-than-pinned tmux may stop receiving the one-time unaudited-version warning even though the installation still checks and requires tmux. Suggested bucket: other for historical diagnostic/authoritative-documentation mismatch; could-be-highest: no established security or work-loss route. Confidence: confirmed source/control-flow and current-at-reviewed-revision contract mismatch, not execution. Proposed_drop: false for that historical finding; prior proposal based on the helper already being unused is withdrawn as a full-range exclusion, while preserving its valid attribution correction that the 1ed deletion is not itself the first lost emitter. No parent drop accepted. Exact contract at reviewed SPEC_impl.md:2882-2889 makes the remainder the installation's current tmux policy and says 'Installation keeps requiring, installing and version-checking tmux exactly as below for now (the installer, the desktop app's preflight, Linux provisioning and `farhelm helm setup`).' The floor paragraph at :2957-2959 says 'a version newer than the pinned one earns a one-time warning that it is unaudited, never a refusal.' It supplies no exception for native desktop preflight or an unqueried dependency. Source: preflight.rs:155-159 maps AtFloor and AbovePin to acceptance with None; :342-378 emits only refusal, and tmux.rs:378-387 returns the pure classification without a warning. Desktop contains no other AbovePin/newer/unaudited emitter. Its preflight still probes and requires this dependency, so the warning promise cannot be satisfied by an absent probe result or by the supervisor that now runs no tmux. Full-range condition/attribution: at exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a core.rs:6630-6636 calls driver adoption. An answering private server reaches the floor check, whose AbovePin arm at base tmux.rs:1197-1203 emits the warning. dbfc removes that production adoption and marks the driver absent; the assigned 1ed patch then deletes this already-unused helper. The base fresh-no-server case already omitted the warning, so do not claim all desktop warning silence first begins here. The narrower previously-warning adopted-old-server installation loses its emitter at the in-range retirement, while the installation-policy text is retained as current at 1ed. Explicit historical-scope caveat: dbfc SPEC_impl.md:2962-2963 says most following prose describes the old driver, and :3005 says 'What follows describes the old terminals as the supervisor ran them.' That can qualify its old operational prose. It does not settle the current-at-1ed mismatch: 1ed removes that historical driver exposition, expressly retains the installation policy, and presents the same unqualified newer-version warning rule within that retained policy. Even if dbfc's warning sentence is read as historical, 1ed's current-policy map remains inaccurate until the Mac requirement goes away. Remedy at that intermediate revision: either make desktop preflight emit the stated warning or state the retained version policy's actual narrower warning behavior. Later repair/scope: 17a3fe8 removes the entire Mac preflight and tmux prerequisite, so no ongoing endpoint Mac-preflight defect is claimed. Possible_cover: parent-supplied g093/l1 historical tmux-prerequisite documentation if the parent judges the same cause; no prohibited coverage source was searched. Caveats: this is not a security claim about running an unaudited terminal substrate, because this supervisor no longer uses tmux. Runtime disappearance, documentation truth and later policy retirement are distinguished. All readily checkable source/contract premises are resolved; no open l2 lead. This revises a known ID from the named pending synthesis, not a new discovery.

Returned proposal (claims-102, event 24): **covered**, suggested bucket **other**.

The historical mismatch is source-established, including newly current authoritative wording at 1ed482d. It is repaired within this same range by deleting the Mac preflight and requirement; no ongoing endpoint warning obligation survives. Preserve the withdrawn attribution-based exclusion and the historical finding in the recording register.

Original stronger consequence: Lost one-time unaudited warning while installation still required/probed tmux; diagnostic/contract correctness, other. The earlier proposal to exclude the whole issue because the helper was already unused was explicitly withdrawn.

Unresolved premise or limitation: None for the historical documentation/control-flow mismatch. Occurrence and frequency were not measured. Fresh no-server installations were already silent at the base, so do not attribute every silent preflight to this range.

Proposed basis: Already repaired within the reviewed range, not source-refuted and not accepted as historically correct. If recording policy intentionally retains already-repaired historical documentation findings, the parent should keep that narrower historical record under this key.

Existing parent disposition (event 24): rejected. Full102: dbfc retires supervisor AbovePin emitter whiledesktopstillclassifiesAbovePinsilently, andnew1edauthoritativeinstallationpolicy retainswarningpromise. Sourceconfirmedintermediatecontract/diagnosticmismatch, not refuted byunusedhelperat1ed or prebasefreshnoserversilence.17a3 deletes preflight/requirement endingtriggerandobligationbeforetip. Preserve originalattributionexclusionwithdrawal; no currentwarningfix or runtimeobservation/workloss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g160 record](records/review-through-00de0b6c/groups/g160.json). Record ID: `l2`.

