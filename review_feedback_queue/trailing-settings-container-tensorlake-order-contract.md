# Settings regression test enforces the wrong section order

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The changed Settings test locks in Containers before Tensorlake, contrary to the
specified navigation order. This remains minor contract drift, not an observed or source-predicted browser-test failure.
Source: settings.spec.ts:31 constructs recipes, containers, tensorlake and line 969 uses ordered toHaveText;
settings.rs:62-75 supplies that same order to the rendering loop at 368-387, so UI and test agree. Synthesis checks:
reread the complete Settings discussion in SPEC.md:2255-2368, recipe/catalog/editor rules at 441-540, full Container
sessions section at 4737-4814, implementation Settings behavior at SPEC_impl.md:934-954, and the explicit
new-environment-pane rule at 6531-6535. Also searched both pinned specifications for Containers/section/pane and
navigation/order exceptions. Result: no rule changes the list order or says grouping providers moves Containers ahead of
Tensorlake. Exact controlling product text at SPEC.md:2255-2257 is “Its left sidebar lists **Sessions**, **Appearance**,
**Hosts**, **Recipes**, **Tensorlake**, **Containers**, **Credentials**, **Terminal**, **Checkouts**, **Agent trust**
and **Notifications**”. SPEC_impl.md:939-941 explicitly says “The order is Sessions, Appearance, Hosts, Recipes,
Tensorlake, Containers, Credentials, Terminal, Checkouts, Agent trust, Notifications”. The more specific new-environment
rule at SPEC_impl.md:6531-6533 says “Settings → Recipes owns the shared catalog/editor and per-place readiness and
Rebuild. Settings → Tensorlake retains account/key/create-confirmation settings and sandbox leftovers. Settings →
Containers ex

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g039/l1`.

### g039/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. TLDR: The changed Settings test locks in Containers before
Tensorlake, contrary to the specified navigation order. This remains minor contract drift, not an observed or
source-predicted browser-test failure. Source: settings.spec.ts:31 constructs recipes, containers, tensorlake and line
969 uses ordered toHaveText; settings.rs:62-75 supplies that same order to the rendering loop at 368-387, so UI and test
agree. Synthesis checks: reread the complete Settings discussion in SPEC.md:2255-2368, recipe/catalog/editor rules at
441-540, full Container sessions section at 4737-4814, implementation Settings behavior at SPEC_impl.md:934-954, and the
explicit new-environment-pane rule at 6531-6535. Also searched both pinned specifications for Containers/section/pane
and navigation/order exceptions. Result: no rule changes the list order or says grouping providers moves Containers
ahead of Tensorlake. Exact controlling product text at SPEC.md:2255-2257 is “Its left sidebar lists **Sessions**,
**Appearance**, **Hosts**, **Recipes**, **Tensorlake**, **Containers**, **Credentials**, **Terminal**, **Checkouts**,
**Agent trust** and **Notifications**”. SPEC_impl.md:939-941 explicitly says “The order is Sessions, Appearance, Hosts,
Recipes, Tensorlake, Containers, Credentials, Terminal, Checkouts, Agent trust, Notifications”. The more specific
new-environment rule at SPEC_impl.md:6531-6533 says “Settings → Recipes owns the shared catalog/editor and per-place
readiness and Rebuild. Settings → Tensorlake retains account/key/create-confirmation settings and sandbox leftovers.
Settings → Containers exposes cached probe results and container leftovers.” It assigns responsibilities and supplies no
ordering exception. Correct the test and UI order together, or settle an intentional contract update. Suggested bucket:
other; the sections are present and usable, with only their order inconsistent. Confidence: confirmed through
source/contract analysis, no execution. proposed_drop: false. could-be-highest: no security or user-owned-work-loss
consequence. possible_cover: none. Caveats: this patch does not itself prove when the UI order first changed, and no
claim is made that the assertion currently fails.

Returned proposal (claims-024, event 9): **retain-verified**, suggested bucket **other**.

The changed ordered assertion enforces Containers before Tensorlake, matching the UI but contradicting both
specifications. The established consequence is minor navigation contract drift, not a currently failing browser test.

Original stronger consequence: Initial lead allowed either deterministic failure against a correct UI or a test
enforcing a specification deviation. Source settles the latter: UI and test currently agree, so the
deterministic-failure alternative is withdrawn.

Unresolved premise or limitation: None for test/spec drift. The precise commit first changing the UI is not established
here and need not be claimed; no browser failure or operational loss is established.

Proposed basis: Record the test and UI ordering mismatch together as other; correct both arrays or obtain an intentional
contract change.

Existing parent disposition (event 9): verified. Parent explicitly accepted named retain-verified proposal from
claims-024 after inspecting it. The changed ordered assertion enforces Containers before Tensorlake, matching the UI but
contradicting both specifications. The established consequence is minor navigation contract drift, not a currently
failing browser test. Record the test and UI ordering mismatch together as other; correct both arrays or obtain an
intentional contract change. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:e2e/tests/settings.spec.ts:31 puts recipes, containers, tensorlake; :969 uses
ordered toHaveText. crates/farhelm-ui/src/settings.rs:62-75 supplies that order in SETTINGS_SECTIONS.","The same
revision SPEC.md:2255-2257 and SPEC_impl.md:939-941 explicitly put Tensorlake before Containers. Current
SPEC.md:2295-2297 and SPEC_impl.md:940-942 retain that order.","The reviewed commit diff newly adds recipes and
containers to the test array. At exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a:e2e/tests/settings.spec.ts:31
neither was in the array. Thus the newly enforced wrong relative order is in range, even without attributing when
production ordering first changed.","plans/queue/podman-environments.md:777-778 and :838-840 authorize updating the test
for extra sections; they do not specify Containers before Tensorlake or override the specifications. No execution was
performed."] Current coverage/results: ["Both current specifications explicitly require the reverse order; no matching
ordering exception found by settings/navigation/order and Containers/Tensorlake searches.","TODO.md Planned is empty.
Matching podman-environments plan sections concern extra sections and retired choices, not accepting a reversed order;
plans/REPORTS.md:21 identifies that plan as already landed. No unlanded exact repair found.","BUGS.md has no matching
issue. INDEX.md and queue searches for section/navigation order and both provider names found no same-root item. Active
TRIAGE_OUTCOMES.md search found no ordering decision. FILTER.md has no boundary matching a persistent ordinary
navigation-order contract inconsistency."] Exact cited cover: [] Original stronger consequence and missing premise
remain preserved: "Initial lead allowed either deterministic failure against a correct UI or a test enforcing a
specification deviation. Source settles the latter: UI and test currently agree, so the deterministic-failure
alternative is withdrawn."; "None for test/spec drift. The precise commit first changing the UI is not established here
and need not be claimed; no browser failure or operational loss is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g039 record](records/review-through-00de0b6c/groups/g039.json). Record ID: `l1`.
