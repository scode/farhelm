# Retained probe fixtures and process oracles are unchanged from the range base

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The added-looking probe tests in the retirement diff do not introduce a new child-lifetime or readiness oracle: the relevant helper and tests are byte-for-byte identical to the excluded range base. Considered the probe fixtures independently from production correctness, including bounded hanging/flooding collection, escaped-descendant group premise and cleanup, descendant-death polling, version/refusal diagnostics, PATH filtering and override precedence. A hypothetical stale recorded PID in the escaped-descendant test could target unrelated processes after the owned descendant exits; could-be-highest: yes for that considered wrong-process signal consequence, but no occurrence or in-range introduction is established. Useful source check: diff -u of base 6e423e30068b62c6cfa276c6d720ee35bb70387a:tmux.rs:6101-6468 against reviewed tmux.rs:831-1198 returned exit 0 with no output. This compares complete pidfile_arg/probe_fixture helpers, all seven test/helper blocks through write_executable, not merely names or the immediate parent. The retained fixture ownership, assertions, timing and documentation in that interval therefore predate the whole pinned range. Confidence: confirmed source equality/attribution; hypothetical unrelated-PID consequence not independently established or observed. Suggested bucket: other for any in-scope test-oracle claim; retain could-be-highest caveat for the considered signal issue rather than lowering it based on rarity. proposed_drop: source attribution only; these bodies are unchanged from the excluded base, and the charter does not ask for independ

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g159/c3`.

### g159/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: The added-looking probe tests in the retirement diff do not introduce a new child-lifetime or readiness oracle: the relevant helper and tests are byte-for-byte identical to the excluded range base. Considered the probe fixtures independently from production correctness, including bounded hanging/flooding collection, escaped-descendant group premise and cleanup, descendant-death polling, version/refusal diagnostics, PATH filtering and override precedence. A hypothetical stale recorded PID in the escaped-descendant test could target unrelated processes after the owned descendant exits; could-be-highest: yes for that considered wrong-process signal consequence, but no occurrence or in-range introduction is established. Useful source check: diff -u of base 6e423e30068b62c6cfa276c6d720ee35bb70387a:tmux.rs:6101-6468 against reviewed tmux.rs:831-1198 returned exit 0 with no output. This compares complete pidfile_arg/probe_fixture helpers, all seven test/helper blocks through write_executable, not merely names or the immediate parent. The retained fixture ownership, assertions, timing and documentation in that interval therefore predate the whole pinned range. Confidence: confirmed source equality/attribution; hypothetical unrelated-PID consequence not independently established or observed. Suggested bucket: other for any in-scope test-oracle claim; retain could-be-highest caveat for the considered signal issue rather than lowering it based on rarity. proposed_drop: source attribution only; these bodies are unchanged from the excluded base, and the charter does not ask for independent pre-range defects unrelated to removal. No parent drop is assumed accepted. possible_cover: none known. Material caveat: equality does not prove the tests or the probe are generally correct; it settles whether this assigned relocation introduced those fixture/oracle concerns. No runtime tests, reproductions or process actions were performed.

Returned proposal (claims-101, event 8): **out-of-range**, suggested bucket **other**.

The complete cited helper and test interval is byte-for-byte identical to the exclusive range base. That settles attribution of the considered fixture/oracle concerns, not their correctness. The hypothetical stale-PID signal route remains explicitly unestablished and must go through the independent drop check.

Original stronger consequence: A stale recorded PID in escaped-descendant test cleanup could signal an unrelated process and lose its work. No occurrence or in-range introduction was established; equality does not prove general fixture correctness.

Unresolved premise or limitation: A concrete stale recorded PID reaching an unrelated live process is not established. Independently, the cited source bodies predate the entire review range.

Proposed basis: Whole-range source attribution only, not rarity, product correctness or acceptance of wrong-process signalling.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g159 record](records/review-through-00de0b6c/groups/g159.json). Record ID: `c3`.

