# Credential delivery can hold a disconnected environment connection

Reviewed commit: 666992f3ac8624f7826779bdefedbd78da92fc54

## TLDR

When an environment disconnects during initial credential delivery, reconnect and launch can stall while Farhelm still
treats the old connection as eligible for replacements.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/manager.rs:4251`.

The supervisor client's natural termination is the persistent closed watch in
crates/farhelm-helm/src/client.rs:3074-3096. The actor's acquisition select waits for either next_nudge or all connected
resources and has no closed branch (crates/farhelm-helm/src/manager.rs:4240-4266). serve later observes closure in its
idle select and through its refresh failure path (5344-5392, 5424-5430, 5529-5543). Explicit hold/removal cancellation
and natural transport closure are distinct. The only production enter calls are the bound Tensorlake and Podman provider
hooks (crates/farhelm-helm/src/tensorlake/hosts.rs:339-355. Crates/farhelm-helm/src/environment/podman.rs:1503-1521).
Ordinary local/SSH hosts bypass the provider hook (manager.rs:4920-4936). Setup uses the serial mutex without acquiring
a connection lease, and replacement owns a task selecting an existing lease (credential_delivery.rs:377-402, 424-476).
On the actor's path, both providers enter before credential delivery. The lease then covers that await, recipe tunnel
acquisition, and serving, with no fixed total lease TTL. CredentialDelivery::connect receives row and events, with no
SupervisorClient, and independently resolves and calls provisioning's route inside a 30-second deadline including its
mutex wait (credential_delivery.rs:273, 362-374, 574-598. Provisioning/backend.rs:21.
Crates/farhelm-helm/src/access.rs:399-450). This allows Tensorlake SSH and Podman exec via either a local or SSH place
to remain pending after the actual supervisor protocol client is closed. After delivery, both providers await the saved
recipe lookup, which has no deadline at the tunnel acquisition site and happens even before the no-tunnels decision. The
spawned recipe relay worker watches client.closed after that lookup, but cannot drop the credential lease
(crates/farhelm-helm/src/tensorlake/tunnels.rs:86-182, 189-207). The 30-second budget bounds the credential await, not
the total pre-serving retention or reconnect delay. A hold nudge can interrupt both awaits. Once the binding is
populated, the old generation remains eligible until the actor drops its lease (credential_delivery.rs:368-370,
431-448). The parent 40d1a549ffe9a1d72100154f59a1c1f9e74bd9ae SPEC_impl.md:5989-5996 and reviewed SPEC_impl.md:5989-5997
specify resource scope and final-drop cancellation. The pinned tip df6199142d6495a95b2d008400ba2686efb775c6 repeats this
at SPEC_impl.md:6002-6010. The statement about final drop is not itself a timer or a natural-closure watcher. Race
resource acquisition against client closure and drop the lease before redial, preserving explicit nudge semantics. A
separate data-delivery plan that preserves this lifecycle does not supply the missing closure boundary. Bucket: high for
delaying the reconnect when an environment's client has already died. No wrong-recipient or credential-loss route is
established. Confidence: source-supported closure and deadline distinction. Actual storage wait duration, remote request
outcome, and production timing remain unobserved. l6's additional acquisition phase is folded into this finding.
Possible coverage: provision/p12 shares the pre-serving lease and hook boundary. This finding remains primary for
natural supervisor closure and replacement eligibility, including reconnects with no attach request. Provision/p12 adds
attach timeout and early Connected publication consequences at the provisioning interface. Those are outside this
finding's established trigger. Credential_delivery_tests.rs:1878-1914 closes the supervisor only after the initial
credential delivery and Connected publication, and proves pre-publication cancellation only via an explicit hold.
Follow-up source check: the recipe lookup calls Db::call (crates/farhelm-helm/src/tensorlake/build.rs:277-298), which
uses a blocking task and a synchronous connection mutex with no deadline on the lock/join
(crates/farhelm-supervisor/src/db.rs:229-243). This establishes that the tunnel acquisition wait has no source-level
wall-clock cap even though the credential phase does. The pinned tip still has the no-closed acquisition select at
manager.rs:4251-4257 and the same Podman ownership order at environment/podman.rs:1529-1547 (l7). Cross-group scope:
provision/p12 is linked from the commissioner's handoff. The original reviewed actor handles acquisition before serving,
and l5's natural-closure consequence does not depend on a provisioning attach waiter or premature publication. No
out-of-scope plan or queue inspection was used.

Additional source record at `df6199142d6495a95b2d008400ba2686efb775c6`, `crates/farhelm-helm/src/manager.rs:4251`.

manager.rs:4240–4266 awaits provider resource acquisition or actor nudge before entering serve. Natural closure
observation is inside serve (5543–5545). Environment hooks own the credential lease and await the independent 30-second
delivery budget (environment/podman.rs:1529–1540, tensorlake/hosts.rs:339–352, credential_delivery.rs:273,285,363–374,
backend.rs:21). Under SPEC.md:489–500, initial delivery precedes usability, a delivery warning leaves a usable
connection usable, and pending pushes are connection-scoped. Provisioning attach waits separately for a new Connected
incarnation and a present client using its 30-second budget (service.rs:60,3412–3505). The shorter remaining attach
window can expire before the old acquisition returns or a fresh connection is published. Ordinary setup and Update use
AttachSupervisor (3406–3409). A fresh environment launch uses the same attach before session creation
(tensorlake_add.rs:697–705,1104–1128) and its ordinary pre-create failure cleanup can remove that launch's own resource
(1145–1186). Update records a failed Attach step without undoing earlier steps (service.rs:3253–3288). An automatic run
remembers the current failure incarnation until a fresh one or a helm restart (3129–3140 and automatic.rs:129–141;
SPEC.md:186–189). This does not permanently block a later genuinely eligible connection. The hold test checks
cancellation during acquisition via nudge and checks natural closure only after Connected
(credential_delivery_tests.rs:1881–1914). It supplies no assertion that an initial hook releases on natural closure.
Bucket: high for a possible user-visible provisioning failure within the documented time bounds. No loss of existing
user work established. Confidence: source-supported ownership and timeout interaction. Actual deadline order, elapsed
delay, and warning/connection publication order on a real transport remain unobserved. proposed_drop: consolidate as one
access/l5 item with this provisioning impact. Keep the source finding. Possible coverage: access/l5. Distinct from p7,
which is about the separate short provisioning probe's helper deadline, and from p4, which concerns pre-admission build
eligibility.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `access/l5`, `provision/p12`.
