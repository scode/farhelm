# Nested build module registers the Podman tests and validates their namespace

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the prior claim that ownership, cleanup and private-input tests never compile or run is refuted by an explicit nested module declaration at both revisions.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g067/l2`.

### g067/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (candidate, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b; also checked pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: the prior claim that ownership, cleanup and private-input tests never compile or run is refuted by an explicit nested module declaration at both revisions. Details: podman.rs:19 declares 'mod build;'. podman/build.rs:9-11 says exactly '#[cfg(test)]', '#[path = "tests.rs"]' and 'mod tests;'. This path resolves tests.rs beside build.rs in podman/, registering the assigned file as environment::podman::build::tests. The earlier searches omitted this nested build module's relative path attribute. tests.rs:4 imports super::* from build, whose line 6 imports its parent podman. The claimed stale namespace is also refuted: tests.rs:527 and 530 use super::super::script_failure_output; from build::tests that correctly resolves to podman::script_failure_output at podman.rs:1488 (1486 at tip). Both missing-registration and stale-namespace claims are withdrawn as factual conclusions, while all prior events remain under this ID. This is direct inclusion of the assigned file, not equivalent coverage inferred from g066's registered failure_tests or its fake structure. Original suspected consequence was wholesale exclusion of the ownership, cleanup and private-input tests; that consequence has no source basis under the actual graph. Suggested bucket: other if the exclusion had existed. Confidence: confirmed by original-revision and tip module/source analysis; no runtime/build execution. proposed_drop: explicit source refutation by the quoted declarations at podman/build.rs:9-11 and their build::tests namespace. possible_cover: none needed; the assigned file is itself registered. Could-be-highest: false. This revision preserves both original claims and their refutation and supplies the exact path-attribute text for the parent's independent drop check.

Returned proposal (claims-042, event 15): **refuted**, suggested bucket **other**.

The alleged orphan file is explicitly included as podman::build::tests. Its reported stale namespace is correct for that actual module graph.

Original stronger consequence: Wholesale exclusion of ownership, cleanup and private-input tests, with suspected compilation errors if wired normally. Both were explicitly withdrawn in later history. This refutation relies on the assigned file's own registration, not substitute coverage from failure_tests. No product security/data-loss route was asserted.

Unresolved premise or limitation: The earlier searches omitted the nested build module's relative path attribute. Both missing-registration and stale-namespace conclusions are contradicted by the actual graph.

Proposed basis: Accept the narrow source refutation while preserving the withdrawn records. Inclusion does not prove compilation/runtime success; it disproves the asserted missing module and wrong super namespace.

Existing parent disposition (event 15): excluded. Parent explicitly accepted named refuted proposal from claims-042 after inspecting it. The alleged orphan file is explicitly included as podman::build::tests. Its reported stale namespace is correct for that actual module graph. Accept the narrow source refutation while preserving the withdrawn records. Inclusion does not prove compilation/runtime success; it disproves the asserted missing module and wrong super namespace. Checked source/results: ["56659839785691f76da3af5d8f51b511cff4ba8b:environment/podman.rs:19 declares mod build; podman/build.rs:9-11 declares #[cfg(test)] #[path = \"tests.rs\"] mod tests. That relative path selects podman/tests.rs alongside build.rs.","tests.rs:4 imports super::* from build, whose line 6 imports podman. Calls at tests.rs:527,530 to super::super::script_failure_output therefore name podman::script_failure_output, not environment.","The same nested module declaration was read at pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:build.rs:9-11. No compile/run was performed; source registration refutes the claimed lack of registration without claiming all tests pass."] Current coverage/results: {"specifications":"Read governing container/ownership/proxy sections. Those contracts do not establish that a test file is excluded; actual module declarations decide it.","TODO.md Planned and plans":"Planned empty; no unlanded entries.","BUGS.md":"Read; no matching orphan-module acceptance.","queue and active triage":"Searched podman/tests.rs, registration, orphan-module and helper symbol terms; no exact coverage. Existing landing-tests-import-unregistered-module index entry is Python tooling with a different module/contract, not this Rust path attribute.","FILTER.md":"No filter invoked."} Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Wholesale exclusion of ownership, cleanup and private-input tests, with suspected compilation errors if wired normally. Both were explicitly withdrawn in later history. This refutation relies on the assigned file's own registration, not substitute coverage from failure_tests. No product security/data-loss route was asserted."; "The earlier searches omitted the nested build module's relative path attribute. Both missing-registration and stale-namespace conclusions are contradicted by the actual graph.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g067 record](records/review-through-00de0b6c/groups/g067.json). Record ID: `l2`.

