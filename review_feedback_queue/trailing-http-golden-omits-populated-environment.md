# The new environment host JSON is omitted from every golden contract row

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The HTTP contract test gains the environment field but leaves its new serialized shape
entirely untested, so changes to the environment key or populated resource metadata can pass the golden check. This is a
concrete test-quality finding; it does not assert that the current serializer is malformed. Revision: promote c2 from
candidate and retract the earlier proposed drop that cited a valid None serializer/shared serde type. Those facts
explain why the golden omits the field; they do not make the test distinguish its populated schema. Original suspected
consequence (malformed container-host metadata passing the HTTP contract tests), prior citations and proposed drop
remain preserved in the original event. Source/attribution: this patch adds environment: None in host_list_body at
http_contract_tests.rs:119-142. Every example, including Tensorlake overrides at :239-264, inherits None and no Podman
row is present. Production HostView adds environment with skip_serializing_if = Option::is_none at hosts.rs:69-71;
unlike the fixture, real host_views at :527-558 populates it, and the exhaustive HostKind projection at :4260-4288
produces Some for both Tensorlake and Podman bindings. EnvironmentHostView at
crates/farhelm-proto/src/environment.rs:143-155 serializes name, place_name, target, tunnels and credential_warning.
Concrete competing regression: adding a serde rename to HostView.environment, while retaining its Rust field and
populated producer, changes actual GET /api/hosts JSON but changes neither the all-None fixture serialization nor
host-list.json; host_list_serializes_to_

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g017/c2`.

### g017/c2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: The HTTP contract test gains the environment field but
leaves its new serialized shape entirely untested, so changes to the environment key or populated resource metadata can
pass the golden check. This is a concrete test-quality finding; it does not assert that the current serializer is
malformed. Revision: promote c2 from candidate and retract the earlier proposed drop that cited a valid None
serializer/shared serde type. Those facts explain why the golden omits the field; they do not make the test distinguish
its populated schema. Original suspected consequence (malformed container-host metadata passing the HTTP contract
tests), prior citations and proposed drop remain preserved in the original event. Source/attribution: this patch adds
environment: None in host_list_body at http_contract_tests.rs:119-142. Every example, including Tensorlake overrides at
:239-264, inherits None and no Podman row is present. Production HostView adds environment with skip_serializing_if =
Option::is_none at hosts.rs:69-71; unlike the fixture, real host_views at :527-558 populates it, and the exhaustive
HostKind projection at :4260-4288 produces Some for both Tensorlake and Podman bindings. EnvironmentHostView at
crates/farhelm-proto/src/environment.rs:143-155 serializes name, place_name, target, tunnels and credential_warning.
Concrete competing regression: adding a serde rename to HostView.environment, while retaining its Rust field and
populated producer, changes actual GET /api/hosts JSON but changes neither the all-None fixture serialization nor
host-list.json; host_list_serializes_to_the_shared_fixture (:317-325) still compares the same value. A nested serialized
key/target representation change is similarly absent from this fixture. This is directly attributed to accommodating a
new public field in the existing contract constructor without an example of its populated shape. Contract: module docs
:10-18 explicitly call these files the HTTP contract and explain that fully populated options are needed to catch
renames; SPEC_impl.md:495-501 describes the golden files' role. SPEC_impl.md Disposable environments, Environment rows
and provider state says 'Host views retain session-owned rows in Hosts, update tracking and ordinary attention.' Other
checks: git grep for EnvironmentHostView, environment_view calls and JSON environment access across crates/e2e at this
revision found only the type, re-export, production declaration and projection; no populated serialization/assertion
test was found. UI Host at this revision does not yet mirror the environment field, so this finding is narrowed to the
producer golden's unpinned public schema; it does not invent an existing UI environment decoder regression. A populated
Tensorlake/Podman golden example would pin the outer key and shared nested serialization; retain None on ordinary hosts
to cover omission too. Suggested bucket: other/test quality. Confidence: confirmed by source and retained assertions; no
tests or reproductions run. proposed_drop: false. possible_cover: none known; no forbidden coverage sources searched.
Could-be-highest: no; the finding establishes test sensitivity, not an established destructive/security path. Caveats:
shared serde types ensure Rust type compatibility, not stable JSON key names; adding a production projection integration
assertion may be useful separately but the concrete in-scope gap does not depend on judging that wider implementation.

Returned proposal (claims-008, event 7): **retain-verified**, suggested bucket **other**.

Every producer golden row omits the newly added public environment projection, so the golden comparison cannot pin its
outer JSON key or populated nested shape. Correct shared serde types do not protect that public serialized contract.

Original stronger consequence: Malformed container-host resource metadata or a renamed environment key could pass the
HTTP golden check. Initial line 2 proposed dropping a product-bug interpretation because None is legitimately omitted
and the nested type is shared; line 7 withdrew that drop and retained the narrower test-quality defect. No
destructive/security route or observed production failure was claimed.

Unresolved premise or limitation: None for the producer golden omission. No malformed production serializer or existing
UI environment decoder failure is established. This consultation does not independently establish the broader historical
assertion that no other serialization test exists anywhere.

Proposed basis: Retain as other/test quality. Add a populated environment example to the producer golden while retaining
ordinary None rows. This is distinct from c1: the failing contract field and necessary fixture/assertion repair differ.

Existing parent disposition (event 7): verified. Parent primary source verification and fully read focused008
source/current-cover checks agree:5665983 adds environment None to every host golden and None is omitted, while
production populates Some for Tensorlake/Podman. The test cannot pin the outer key or nested populated shape; shared
serde types and lack of a current UI decoder do not refute that bounded public serialization oracle gap. No malformed
production serialization or loss of work claimed. Current specifications require the shared contract boundary, with no
exact Planned/unlanded-plan/BUGS/queue/active-ledger or filter cover. Retain separately from epoch0/defaulted consumer
because the failing contract field and necessary oracle differ.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g017 record](records/review-through-00de0b6c/groups/g017.json). Record ID: `c2`.
