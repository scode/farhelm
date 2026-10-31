# A test UUID hold can intercept a terminal lease

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The destination-authority test's global UUID hold can catch late terminal startup instead of the intended launch key.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The test replaces the shared UUID source before all terminal lease minting is necessarily complete. Source checks
establish two relevant UUID consumers and a conditional ordering, but no actual adverse browser scheduling was observed.
The retained record identifies the fixture premise and native/renderer ordering evidence still needed.

Source locations: `e2e/tests/destination-authority.spec.ts:256-278`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: the post-key-mint regression can count terminal startup
work as part of its held create operation and fail for fixture timing instead of destination authority. Source basis:
the fixture globally replaces Crypto.prototype.randomUUID and increments one undifferentiated counter, then requires
entered==1 (destination-authority.spec.ts:256-278). Both create keys and terminal leases call CSPRNG_ID_JS through
eval_minted_id (api.rs:3041-3042,3076-3080,3105-3113,3131-3139). historyFixture waits for the feed, hosts and form, not
startup session selection/lease completion (test:108-157). Actual shipped independent producer: ListView awaits a fresh
preferences GET before automatic selection (list/view.rs:3064-3138); its completion does not check show_create. Opening
New checks the operation lock but does not claim it (view.rs:3614-3643; ops.rs:100-132). App mounts SessionView after
that selection (lib.rs:1948-1951,2020-2063); its mount future mints a lease (session_view.rs:757-770). If the startup
selection completes after override installation and before Launch claims the operation lock, the lease can enter the
hold; the create then supplies a second call, so entered can become 2. The busy check after Launch does prevent a newly
admitted automatic selection then; this finding does not ignore that guard or claim every startup interleaving is
possible. Missing dynamic premise: the independent preference-read/render/lease continuation actually spans the
fixture's setup and lands in that pre-Launch interval on the browser substrate. No such ordering or failure was
observed. Actual accessible checks: traced both UUID consumers, selection completion, New admission and App mount;
compared the entire test at cc734789f2ce77155ad2838eea7771bd52fe240c and found the same override; searched SPEC.md,
SPEC_impl.md, empty TODO Planned bucket, BUGS.md, plans/queue, queue INDEX and referenced startup/font items, and
TRIAGE_OUTCOMES.md. No exact coverage. Source cannot establish the runtime scheduling/frequency of the pending
network/render work under the no-experiment review constraint. Evidence that would settle it: a retained controlled run
holding the startup preference read, installing the UUID override, then releasing the read before Launch and recording
whether the lease and create both enter; a matching bounded readiness oracle before override installation would rule
that route out. Suggested fix is settle the intended initial session's lease before overriding, or scope the injected
hold to the create-key operation. Bucket other; confidence likely conditional test-fixture interference with the named
dynamic premise unverified. No false pass, product bug, vendor turn, security effect or user-work loss is claimed.
proposed_drop: none. possible_cover: prompt-prefill-test-opens-before-source-selection.md documents a different
starting-folder failure in templates/clone specs; held-font-test-lacks-mount-attempt-witness.md concerns an independent
font test oracle; catalog-cleanup and TRIAGE_OUTCOMES.md:20197-20209 cover pre-POST work after observation removal, not
this counter/hold interference. Rare-safe-retry FILTER applicability would need actual rarity and recovery evidence and
is not established here. Attribution: inherited at cc734789f2ce77155ad2838eea7771bd52fe240c, not caused by sentence-case
selectors in 08aaac8a4a5bcf91726a1d3bfcfc980de4116971. Log ID f2.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g028/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
