# Failure at an enumerated place suppresses healthy container leftovers

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: one failing enumerated place hides healthy places' unregistered leftovers and cleanup
controls, including previously displayed rows. Original cleanup obstruction and conditional high suggestion are
retained; suggested other remains only a recommendation, with no final priority/drop decision. Exact enumerated set:
cached-usable ordinary Local/SSH hosts, every environment row's place_host, and every recipe place whose state
needs_cleanup (podman.rs:777-792). A merely registered unusable/unprobed host without references is excluded. list_owned
failure at 795 discards healthy results; list_leftovers returns only 502 at 854-860. At the later revision,
containers_settings.rs:22-30 overwrites catalog on error and 98-125 hides its rows, while api/environments.rs:63-71 has
only a global listing. Source contract: SPEC.md:4809-4814 calls for the sandbox-leftovers pattern; that pattern at
521-531 expressly says a failed read keeps earlier rows. SPEC_impl.md:7174-7201 also requires failed reads to retain
rows in its detailed unowned-sandbox model. Ownership in the database does not retain cached displayed rows. Fallback
checks completed: Podman clean_images (environment/podman/build.rs:764-803) visits recorded state.podman places,
reconciles only recorded builder/preparer coordinates (630-675), and retires recorded images (689-759). It does not
discover or remove unregistered role=session containers. Its non-forcing image retirement deliberately leaves images
used by ANY container (721-737), so recipe deletion cannot delete an unregistered session as a substitute. Hosts removes
row-back

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g065/l2`.

### g065/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b; later unchanged provider and production UI checked at
ba7cc0264028eebb39647deb1d25b055e6886935. TLDR: one failing enumerated place hides healthy places' unregistered
leftovers and cleanup controls, including previously displayed rows. Original cleanup obstruction and conditional high
suggestion are retained; suggested other remains only a recommendation, with no final priority/drop decision. Exact
enumerated set: cached-usable ordinary Local/SSH hosts, every environment row's place_host, and every recipe place whose
state needs_cleanup (podman.rs:777-792). A merely registered unusable/unprobed host without references is excluded.
list_owned failure at 795 discards healthy results; list_leftovers returns only 502 at 854-860. At the later revision,
containers_settings.rs:22-30 overwrites catalog on error and 98-125 hides its rows, while api/environments.rs:63-71 has
only a global listing. Source contract: SPEC.md:4809-4814 calls for the sandbox-leftovers pattern; that pattern at
521-531 expressly says a failed read keeps earlier rows. SPEC_impl.md:7174-7201 also requires failed reads to retain
rows in its detailed unowned-sandbox model. Ownership in the database does not retain cached displayed rows. Fallback
checks completed: Podman clean_images (environment/podman/build.rs:764-803) visits recorded state.podman places,
reconciles only recorded builder/preparer coordinates (630-675), and retires recorded images (689-759). It does not
discover or remove unregistered role=session containers. Its non-forcing image retirement deliberately leaves images
used by ANY container (721-737), so recipe deletion cannot delete an unregistered session as a substitute. Hosts removes
row-backed resources only, and an unregistered leftover has no such row. Thus healthy-place unregistered session
leftovers have neither a Hosts nor recipe-cleanup fallback in the shipped UI. They remain removable through manual
Podman operations or direct DELETE with independently known canonical ID/place; those are external workarounds rather
than UI discovery. Fix direction: retain previous rows with an error and return per-place healthy discoveries plus
explicit incomplete-place diagnostics; do not silently treat failure as absence. Suggested bucket: other, unaccepted
recommendation; could-be-highest: no established security or loss-of-work route. Confidence: confirmed source/contract
analysis; all readily checkable discovery, cache-retention and fallback premises resolved. proposed_drop: false.
possible_cover: none; prohibited coverage sources not searched. No runtime tests, builds or reproductions ran.

Returned proposal (claims-041, event 8): **retain-verified**, suggested bucket **other**.

A failure at an enumerated place aborts the whole leftovers response, hiding healthy places' unregistered containers. A
failed refresh also replaces earlier successful UI rows with an error. No shipped UI alternative discovers those
unregistered session leftovers; this establishes cleanup obstruction, without establishing session disruption or loss of
user work.

Original stronger consequence: The initial lead conditionally suggested high if healthy-place cleanup lacked an
alternative; subsequent source checks establish that UI obstruction but recommend other because the proven consequence
remains cleanup/discovery. No highest/security/user-work-loss route was alleged. The correction from any registered host
to the exact enumerated set must be preserved.

Unresolved premise or limitation: None for healthy-place UI cleanup obstruction and loss of displayed rows. A session
disruption, security consequence or destruction of user work is not established. Hosts remains available for row-backed
resources, which is a narrower fallback than unregistered discovery.

Proposed basis: Retain the complete discovery and cached-row contract defect as other. Preserve per-place failure
diagnostics in any repair; silently skipping a failed place would falsely promise complete discovery.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from
claims-041 after inspecting it. A failure at an enumerated place aborts the whole leftovers response, hiding healthy
places' unregistered containers. A failed refresh also replaces earlier successful UI rows with an error. No shipped UI
alternative discovers those unregistered session leftovers; this establishes cleanup obstruction, without establishing
session disruption or loss of user work. Retain the complete discovery and cached-row contract defect as other. Preserve
per-place failure diagnostics in any repair; silently skipping a failed place would falsely promise complete discovery.
Checked source/results: ["56659839785691f76da3af5d8f51b511cff4ba8b: environment/podman.rs:777-792 enumerates
cached-usable ordinary hosts, every environment-row place_host and recipe places needing cleanup. A merely registered
unusable/unprobed unreferenced host is excluded.795 propagates any list_owned error, losing results and skipping later
places;843-860 returns only502.","Same revision, list_owned1338-1380 and require189-202 preserve command/inspection
errors, while podman_probe runtime338-378 can return host/probe errors. Thus an unavailable referenced place can trigger
this condition, without pretending every registered offline host necessarily
does.","ba7cc0264028eebb39647deb1d25b055e6886935: containers_settings.rs:22-30 overwrites catalog even on error;98-125
renders an error instead of all former rows/Delete controls. api/environments.rs:63-71 has one global listing,
while76-89 needs a discovered host row or independently known place/ID for deletion.","56659839:
environment/podman/build.rs:630-675 reconciles only recorded builder/preparer helpers;764-803 cleans recorded recipe
images/places.721-737 retains images used by any container. It neither discovers nor deletes unregistered role=session
leftovers. Hosts requires a row; manual Podman and direct DELETE with independently known canonical coordinates are
external workarounds.","Endpoint diff leaves affected provider listing unchanged. Source/caller checks only; no outage,
UI refresh or deletion reproduction ran."] Current coverage/results: ["Current SPEC.md:4831-4838 requires
interrupted-resource discovery, and4888-4890 requires the sandbox-leftovers pattern. That pattern528-539 explicitly
retains earlier rows after a failed read. SPEC_impl.md:6087-6092 requires label discovery at cached-usable and
referenced places. None explicitly accepts cross-place suppression or discarding prior rows.","TODO Planned is empty.
Searched plans/queue for leftovers/failure/discovery/container settings; podman-environments.md:94-95,246-250 states the
feature rather than scheduling repair of its all-or-nothing behavior. environment-agent-operations changes session
deletion, not unregistered leftovers discovery. Other leftover plans concern Git cache locks or different session-action
state.","BUGS.md is the SSH-transfer helper hang. INDEX.md and exact queue-file/source-name searches found no same-root
item. Active TRIAGE_OUTCOMES.md has no container-leftovers decision. FILTER.md's rare/self-correcting boundary does not
match the full claim: an ordinary unavailable place can continue suppressing healthy-place discovery and controls until
that place recovers or its references change; repeated refresh alone need not repair it."] Exact cited cover: []
Original stronger consequence and missing premise remain preserved: "The initial lead conditionally suggested high if
healthy-place cleanup lacked an alternative; subsequent source checks establish that UI obstruction but recommend other
because the proven consequence remains cleanup/discovery. No highest/security/user-work-loss route was alleged. The
correction from any registered host to the exact enumerated set must be preserved."; "None for healthy-place UI cleanup
obstruction and loss of displayed rows. A session disruption, security consequence or destruction of user work is not
established. Hosts remains available for row-backed resources, which is a narrower fallback than unregistered
discovery.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g065 record](records/review-through-00de0b6c/groups/g065.json). Record ID: `l2`.
