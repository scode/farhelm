# Version documentation omits the schema Update guard

Reviewed commit: 29c02d1e35ee2380ee571d4aaa748fe63e166ed5

## TLDR

The implementation specification's version/skew description omits the schema-based guard that can forbid Update. The
product specification and actual guard remain intact.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `SPEC_impl.md:5041-5053 at 29c02d1e35ee2380ee571d4aaa748fe63e166ed5`. Locations and revisions in the
assessment below identify the reviewed source; later main is outside this review's pinned range.

Reviewed assigned commit: 29c02d1e35ee2380ee571d4aaa748fe63e166ed5. TLDR: the authoritative implementation description
does not explain the separate host-database-schema Update refusal or the optional handshake information it uses. The
documentation gap survives approval of the plan that reported it. Details: SPEC_impl.md:5041-5053 at the assigned
commit, unchanged in substance at pinned tip 463cac1528b5c5e259404975380d321a3fff0412:5050-5062, describes
protocol/build hello information and version ordering without the schema mechanism. The deleted parent report
eb305d457577b25019fc793bbd2c4367978b2839:plans/reports/update-refuses-newer-host-schema.report.md:111-112 expressly
identifies this missing implementation description. Add narrow implementation context explaining the optional reported
schema, the independent guard in both protocol paths, revalidation before installation, and legacy peers that report no
schema. Suggested bucket: other. Confidence: confirmed documentation omission. Caveats: this omission predates the
assigned approval deletion; approval itself follows the documented lifecycle, and does not remove the product guard. The
sentence that development helms never refuse Update as a build-version downgrade is not literally refuted by a separate
schema refusal. Pinned-tip provisioning/service.rs:268-285,1081-1094,1127-1139,1866-1887,1913-1929 still refuses a
reported higher schema at planning and confirmation. provisioning.rs:2214-2303 pins both protocol paths, equal/missing
acceptance and no backend operations after confirmation refusal; source inspected only, no runtime pass claimed.
SPEC.md:119-125 and TRIAGE_OUTCOMES.md:18743-18759 retain the behavior and waiver. No unsafe update, data loss or
security consequence is inferred from absent prose. Possible_cover: the SPEC and completed ledger retain the underlying
guarantee, but do not expressly accept or schedule this exact implementation-documentation omission. No matching current
unlanded plan, Planned TODO item (bucket empty), BUGS entry, queue item or exact ledger acceptance was found. The
unrelated rollback TODOs, retry-retention queue finding and earlier version-downgrade ledger outcome do not match the
documentation omission's trigger, consequence and scope. Proposed_drop: none for the narrow documentation residual; its
origin before this assigned deletion is explicit for the parent's scope/materiality judgment. This revision narrows
attribution and supplies exact original-commit lines; it does not expand the claim to a production update defect.

Provenance: `g118/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
