# Removed ssh host can reappear with old grants

Reviewed commit: 8a8c9579e04299410652f0e4681975a7198dfc7b

## TLDR

A power loss can roll back ordinary SSH-host removal and restore its old identity, connection authority, and grants.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location:
`crates/farhelm-helm/src/store.rs:7344-7382 at 771864ac3f38334a8191a424d855669efddb089a;7532-7571 at 8a8c9579e04299410652f0e4681975a7198dfc7b`.

Source assessment (read with the later corrections below) (review record g076/f2, event 15):

Reviewed commit: 771864ac3f38334a8191a424d855669efddb089a; synthesis rechecked through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: After Remove returns success, a helm-machine OS crash or power loss can
bring a trusted SSH host back with its old permission to control the fleet without a new Allow answer. This includes an
ordinary Linux helm and does not depend on the separately accepted Mac drive-cache limitation. Production
acknowledgement: introducing hosts.rs:1229-1235 awaits remove_host_for and returns Json({}) only on Ok; 1247-1249 and
1337-1357 await removal/teardown. Pinned equivalents are hosts.rs:1247-1267,1355-1375. provisioning/service.rs:1936-1954
(pinned 1939-1957) deletes through the store before retiring process-local memory/actor state. store.rs:7350-7382
(pinned 7538-7571) calls ordinary Db::call and DELETE FROM hosts, without the critical barrier. Pinned
store.rs:4995-5014 configures the real opener through db::configure_durability, whose 110-115 sets NORMAL for actual
WAL; this shared db source is unchanged. The four explicit permission setters use FULL/EXTRA, so an already durable
baseline grant followed by this unsynced removal is a concrete sequence. Failure requires loss of that recent NORMAL
deletion before a later sync/checkpoint makes it durable; it is not a mere process crash. The remote SSH supervisor and
its sessions are left untouched by Remove (introducing hosts.rs:1196-1201 and SPEC.md Topology). On restart, production
lib.rs:2359-2398 constructs SystemTransport/ConnectionManager and installs HelmAgentRequests.
manager.rs:1452-1463,1665-1668,1838-1844 (pinned 1466-1478,1679-1682,1852-1859) creates actors from the restored
registry. SystemTransport at transport.rs:137-146,182-219 uses the same destination and existing SSH access;
ssh.rs:439-455 invokes remote internal stdio. Existing SSH authentication need not ask the user again for an ordinary
already-configured host. manager.rs:4156-4187 (pinned 4185-4216) binds the client to the row; store.rs:7443-7458 (pinned
7631-7645) accepts unchanged endpoint/identity as Recorded. The duplicate guard can be satisfied by an ordinary unique
entry. manager.rs:4344-4357,4058-4084,4479-4489 (pinned 4373-4387,4087-4113,4508-4519) configures and publishes that
same authenticated supervisor. client.rs:1843-1849,2459-2464,2752-2797 gives its upcalls a new connection-bound
AgentOrigin, and agent_requests.rs:391-422,946-952 verifies that exact current client. These gates do not revoke a
restored durable permission. approvals.rs:411-443, byte-identical at both revisions, reloads commands_without_asking and
returns Allowed::Raw before the no-GUI refusal. Pinned agent_requests.rs:487-513 consumes this for Stop and
sessions.rs:2484-2489 actually calls the target supervisor's stop_session. A removed A can therefore stop a connected
B's running agent without a new approval. Create reaches approve_launch and dispatch_agent_create (introducing
agent_requests.rs:2287-2367,2568-2644; pinned approve_launch answer/recheck 2358-2374), with final target checks in
pinned sessions.rs:1655-1667. yolo_guard.rs:124-168 is unchanged: ordinary guarded structured launches pass, while
arbitrary command/YOLO launches require B's own yolo_without_asking=true. That independent target policy remains intact;
the bug is restored caller authority. Browser effects require the native link_opener for clickless opening
(approvals.rs:430-436), so the Linux witness rests on fleet commands, not a presumed native browser capability.
Suggested bucket: highest, because restored remote authority can execute admitted launches or terminate user-owned
processes across hosts. Confidence: confirmed source mechanism and production consumers, conditional on documented
possible NORMAL commit loss; no runtime power-loss experiment was run or needed. proposed_drop: false. Exact
possible_cover for the parent's independent check: SPEC_impl.md at introduction 5100-5107 / pinned 5138-5145 names host
registry state, selects ordinary WAL/NORMAL, and says 'This accepts losing the last commits on an OS crash or power
loss'. That text is broad, not expressly limited to unrelated metadata. However the complete same bullet at introduction
5110-5112 / pinned 5148-5150 says the named trust-changing paths use the barrier 'so a power loss cannot restore
acknowledged trust revocations' and 'Unrelated preference patches remain NORMAL'. Neither revision explicitly accepts
host-removal rollback or resurrected remote execution authority. The complete SPEC.md Local authority section
(introduction 4010-4122; pinned 4143-4255) still requires remote execution authority to be user-granted. Do not silently
stretch general registry rollback into a specific security acceptance; retain this exact ambiguity for the parent.
Complete SPEC.md Durability and resume (introduction 2697-3000; pinned 2823-3130), Healthy local filesystems/Power-loss
download/checkouts sections, both implementation SQLite durability bullets, and complete SPEC_impl Permission prompts
(introduction 7952-8201; pinned 8106-8355) were reread during synthesis. The pinned additions accepting recent
conversation loss and full-disk Resume restoration failure do not explicitly authorize resurrected host trust. Mac
FULL/EXTRA reaching only drive cache is explicit, separate coverage for f1, not for this Linux omission. Caveats:
omitted pre-existing revocation path adjacent to the assigned setter fix, not newly introduced removal logic. A rollback
of adoption is excluded because identity and permission reset roll back together and mismatch blocks a successor; this
finding uses the same still-running old host. No fix through pinned tip: exact
removal/acknowledgement/reconnect/permission consumers remain; approvals/client/yolo_guard/transport/ssh source is
byte-unchanged over the reviewed tail, and pinned manager/store relevant sites were inspected. Suggested fix: give
removal the critical FULL/EXTRA barrier or durably revoke its authority before acknowledgement, with production-path
regression coverage. No queue/plan coverage searches, runtime tests, VCS mutations or discovery pass. All earlier events
are retained. Synthesis has no unresolved source lead; the specification-boundary decision is explicitly the parent's
independent check.

Later qualification (review record g076/f2, event 16):

Reviewed commit 771864ac3f38334a8191a424d855669efddb089a, implementation checked through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b in prior synthesis. TLDR unchanged: power loss can restore an acknowledged
removed SSH host and its old fleet authority. Suggested bucket highest; confidence confirmed source route conditional on
documented NORMAL commit loss; proposed_drop false. Exact existing source route and qualifications remain in f2's
preceding complete synthesis event: production Remove acknowledges after Db::call/NORMAL deletion and actor teardown;
the same restored UUID/identity/destination passes SSH/identity/current-origin checks at startup; approvals::ask
consumes restored commands_without_asking for cross-host Stop and admitted creates. This is a duplicate of existing
g076:f2, not discovery. Supplied owner g005:l2 (g005/findings.jsonl:7) corrects its earlier removal/adoption conflation:
only atomic adoption-transfer is refuted. Its directly reviewed scope is plans/queue/durability-revocations.md:80-84 at
planning commit 5d9e4d94aa5063a6876789228d1aab8899180770, with contracts checked through pinned tip. g005 says the
host-removal/adoption exclusions occur in Planner proposals, not Decisions; Background:20-24 distinguishes protecting
trust from recent history. This planning-source classification is attributed to g005, not independently expanded or
searched here. Supplied owner g029:r7 (g029/findings.jsonl:22), reviewed 771864ac3f38334a8191a424d855669efddb089a,
refutes missing barriers only in store.rs:5591-5596,6983-7021,7030-7068,7162-7199 with db.rs:154-188,230-266. It
explicitly does not refute g076:f2 or all trust-revocation routes; different-entry retry UUID repair cannot protect
restoration of the same entry. Supplied owner g030:f1 (g030/findings.jsonl:23), reviewed
33eb4382cdc55bc8758a7e1794072785e11f1325, keeps only the Mac FULL/EXTRA drive-cache exception as a highest proposed drop
pending independent consultation; it does not accept missing software barriers or weaken other-platform security.
possible_cover remains the exact general registry/NORMAL wording at introducing SPEC_impl.md:5100-5107 / pinned
5138-5145, contrasted with durable trust revocations at introducing 5110-5112 / pinned 5148-5150; g005 supplies evidence
against interpreting a planning exclusion as accepted removal rollback. Parent owns independent specification/coverage
judgment. No new finding, runtime evidence, planning inventory read, or independent Mac drop decision is claimed.

Additional source evidence (g012/l6, event 17):

Reviewed commit b11808ce0717a97294b8c6b1c7c7ee431cabacec. TLDR: the maintainer's landing report says every withdrawal of
the named permissions was checked for strong power-loss durability, though the verified repair names setting writes and
host removal still uses the ordinary database path. Bucket other; confidence confirmed for this report/source scope
mismatch, not for an independently established security consequence of the wording. Report lines 108-109 say every place
these permissions and tunnel consent are revoked takes the stronger barrier; lines 93-98 summarize revocations broadly.
SPEC_impl.md Helm internals:5103-5120 names command/YOLO/browser permission setters and present tunnel-consent patches,
while ordinary WAL writes remain NORMAL. Store remove_host_row at :7344-7382 uses Db::call (:7350-7353), deletes the
trusted host row (:7365-7368), and permits SSH removal (:7378). SPEC.md host registration:236-237 says removing a
registry host leaves the supervisor untouched. Narrow the report to the named setter/patch paths and do not infer a
universal withdrawal guarantee. Temporal check now completed: git log over b11808ce..8a8c9579 for this exact report
returns no later modification; at pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b remove_host_row still uses .call
at store.rs:7532-7570, with DELETE at :7554. This is an interface-derived duplicate/qualification of known g076-f2 as
explicitly preserved in owner g029:r7@22, not new runtime discovery. The narrower r7 setter refutation stands; the exact
accepted durability scope and security consequence of g076-f2 remain the owner's/parent's judgment. No local rollback
experiment, runtime execution, or g076 history search was performed. proposed_drop false; possible_cover: known
underlying route g076-f2, supplied via g029:r7@22; parent owns semantic deduplication and coverage. This event resolves
the temporal follow-up in the preceding l6 finding and preserves the original lead/history.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g076/f2`; supporting records
`g012/l6`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
