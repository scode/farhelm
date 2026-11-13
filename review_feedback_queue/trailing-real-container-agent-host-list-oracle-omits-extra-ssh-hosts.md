# Real-container hosts-list test overstates its exact-membership oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the real-container test says its hosts listing shows only that container, but its assertions can pass while extra ordinary SSH hosts appear. This creates false assurance about the real CLI/relay trust-boundary test, without establishing an actual production escape. Suggested bucket: other (test-oracle/documentation correctness); confidence: confirmed by source, not execution; proposed_drop: false. Could-be-highest: no established security or user-work-loss route. possible_cover: agent_requests.rs:14518-14526 separately checks exact handler response cardinality/name/current, but does not prove this real CLI output oracle; no prohibited coverage sources were searched. The newly added helper's docstring at lines 696-702 claims the hosts listing shows only this container. Lines 788-797 merely require own_name as a substring and exclude the substring this machine; they do not parse returned host rows or forbid ordinary SSH host names. The SSH variant includes another ordinary registered host. The competing bad output containing the container plus that SSH host satisfies both assertions, so the limitation is concrete without a reproduction. Either parse and assert the single returned host identity, or narrow the comment/diagnostic to own inclusion and local-host exclusion. Correct production filtering and the separate handler test do not refute the distinct real-test/comment defect. This revises the earlier proposed drop under the same ID; the original false-assurance consequence is retained and the earlier proposed bases (correct filtering and complementary handler coverage) are

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g070/c4`.

### g070/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 503af78794bf61361a10eed3295b563c87289228. TLDR: the real-container test says its hosts listing shows only that container, but its assertions can pass while extra ordinary SSH hosts appear. This creates false assurance about the real CLI/relay trust-boundary test, without establishing an actual production escape. Suggested bucket: other (test-oracle/documentation correctness); confidence: confirmed by source, not execution; proposed_drop: false. Could-be-highest: no established security or user-work-loss route. possible_cover: agent_requests.rs:14518-14526 separately checks exact handler response cardinality/name/current, but does not prove this real CLI output oracle; no prohibited coverage sources were searched.

The newly added helper's docstring at lines 696-702 claims the hosts listing shows only this container. Lines 788-797 merely require own_name as a substring and exclude the substring this machine; they do not parse returned host rows or forbid ordinary SSH host names. The SSH variant includes another ordinary registered host. The competing bad output containing the container plus that SSH host satisfies both assertions, so the limitation is concrete without a reproduction. Either parse and assert the single returned host identity, or narrow the comment/diagnostic to own inclusion and local-host exclusion. Correct production filtering and the separate handler test do not refute the distinct real-test/comment defect. This revises the earlier proposed drop under the same ID; the original false-assurance consequence is retained and the earlier proposed bases (correct filtering and complementary handler coverage) are explicitly not accepted as refutations. The later 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa Delete-oracle issue supplied as g067/l1 is distinct. This helper is introduced within the pinned range at the reviewed commit, not attributed to earlier unrelated test code. No runtime tests or reproductions.

Returned proposal (claims-044, event 13): **retain-verified**, suggested bucket **other**.

The helper's stated exact-membership contract exceeds its real CLI assertion. Output containing both the container and an ordinary SSH host satisfies own-name inclusion and local-label exclusion. Correct product filtering and a separate exact handler test do not refute this test/comment defect.

Original stronger consequence: Line 6 proposed dropping the real-route gap because production filtering and handler cardinality were correct. Lines 11 and 13 explicitly reverse that basis and retain false assurance in the real CLI/relay oracle. Preserve that reversal. The separate g067/l1 Delete-authority oracle and g070/l1 fallback lifetime concern are not refuted by this handler test.

Unresolved premise or limitation: No missing source premise for the assertion/comment mismatch. An actual escaped production host listing is not established or required for this test defect; no runtime false pass was observed.

Proposed basis: Retain as other for established test-oracle/documentation correctness. Because earlier proposed dismissal relied on complementary production security-boundary checks, send any parent decision to revive that dismissal or lower a stronger security interpretation through the independent drop check. No actual production security consequence has been demonstrated.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-real-CLI-host-membership-oracle-overclaim. Parent read full044. New503 real helper says only this container but checks own-name substring and absence of local label; own plus ordinary SSH host passes. Separate production scope filter and exact handler test are complementary and do not establish this real CLI/relay oracle. Retain source-confirmed test/comment defect at proposed other, no actual escaped production listing/security consequence or runtime false pass asserted. Original production-correctness dismissal was explicitly reversed and stays in register. Distinct067l1 Delete-authority oracle and070l1 cleanup lifetime remain unaffected; no exact cover/filter.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g070 record](records/review-through-00de0b6c/groups/g070.json). Record ID: `c4`.

