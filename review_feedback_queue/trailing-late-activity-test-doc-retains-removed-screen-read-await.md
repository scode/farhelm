# Late-activity test documents a screen-read await removed by the host migration

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the changed late-activity regression test teaches future maintainers that reading a
terminal screen yields across a Restart, although the host-only sampler now reads its cached screen synchronously. The
shared activity-cell invariant and the synthetic late-write test remain useful; this is a stale explanation of the
lifecycle window, not a product or test-oracle failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g115/l2`.

### g115/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: the changed late-activity regression test teaches
future maintainers that reading a terminal screen yields across a Restart, although the host-only sampler now reads its
cached screen synchronously. The shared activity-cell invariant and the synthetic late-write test remain useful; this is
a stale explanation of the lifecycle window, not a product or test-oracle failure. Details: core.rs:29636–29646 says
sample_pass 'clones an entry, awaits a read of its terminal's screen, and only then dates what it saw', and explains a
restart in 'that window'. The assigned patch changes this test's entry_info assertion to its host-only signature at
29725, leaving that present-tense mechanism explanation. At this revision ticker.rs sample_pass (1292–1424) clones
entries before earlier async persistence/observation, but the selected screen read is now
`let tail = terminal.screen_tail(SAMPLE_TAIL_BYTES)` with no await; title and activity observation also run
synchronously. hosts/terminal.rs:2229–2240 defines screen_tail as a synchronous fn returning String from the runtime's
emulator/view. The parent-to-reviewed ticker diff explicitly deletes terminal.screen_tail(sup, SAMPLE_TAIL_BYTES).await
and replaces it with the synchronous host method, also deleting the async title read. Therefore the named screen-read
suspension boundary no longer exists. Earlier awaits and multi-worker scheduling can still leave an older cloned entry
alive across Restart; those explain why the session-wide activity cell is still needed and why the test must not be
deleted on this finding. Fix the docstring to describe that actual cloned-entry lifetime rather than an awaited screen
read. Range attribution checked: at exclusive-base 6e423e30068b62c6cfa276c6d720ee35bb70387a the same test at
core.rs:34608–34623 described an awaited capture-pane round trip, consistent with the old sampler. In-range history
identifies 70bc66a9f285cc2a9984924f389834d118b7cfdc (test migration) and this 1ed482d driver removal for the relevant
screen-read/prose changes. Do not dismiss this solely because the comment predates the assigned commit: the reviewed
commit removes the behavior it still describes. Governing documentation rule: explanatory test comments must capture the
lifecycle/async assumptions future readers need; SPEC_impl.md Terminal host describes status sampling from the host
runtime's emulator, not a tmux capture round trip. Suggested bucket: other (internal documentation correctness).
Could-be-highest: no established security or user-work-loss route from this explanation. Confidence: confirmed by source
and parent/base comparison; no execution. proposed_drop: false. possible_cover: supplied neighbour g132/c3 is an
analogous stale ticker-field/failed-capture-reset explanation, but this is a distinct changed-test location and
different screen-read-await claim; parent may consolidate while preserving provenance. Excluded coverage sources were
not searched. Material caveats: no claim that synchronous code cannot be preempted, that all sampler awaits disappeared,
or that the late-write test is invalid. Later in-range repair check remains to be performed next; the charter includes
the intermediate revision either way. Later source check completed: pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
core.rs:29580–29588 retains the same awaited-screen-read explanation verbatim; no later repair found. Exact earlier
prose attribution is 70bc66a9f285cc2a9984924f389834d118b7cfdc, whose core.rs diff replaces capture-pane round trip with
terminal screen read while moving this test to host_supervisor. The later assigned 1ed482d diff demonstrably removes the
remaining async sampler API. The lead is fully resolved; there is no unverified source premise.

Returned proposal (claims-074, event 15): **retain-verified**, suggested bucket **other**.

The test's explanation names an awaited screen read that the reviewed sampler no longer performs. The session-wide
activity-cell invariant and synthetic late-write assertion remain useful.

Original stronger consequence: Maintainers are given an obsolete async screen-read rationale for retaining a
session-wide activity cell. This has consistently been an internal documentation defect, without product failure, an
invalid test oracle, security impact or user-work loss.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the screen-read-await explanation only; preserve earlier sampler awaits, parallel scheduling, and
the useful late-write test. The supplied g132/c3 analogy concerns different removed failed-capture reset prose and is
not an assigned alias established here.

Existing parent disposition (event 15): verified. Parent explicitly accepted named retain-verified proposal from
claims-074 after inspecting it. The test's explanation names an awaited screen read that the reviewed sampler no longer
performs. The session-wide activity-cell invariant and synthetic late-write assertion remain useful. Record the
screen-read-await explanation only; preserve earlier sampler awaits, parallel scheduling, and the useful late-write
test. The supplied g132/c3 analogy concerns different removed failed-capture reset prose and is not an assigned alias
established here. Checked source/results: ["At 1ed482dde220452e424973faf38a5e1318986333 core.rs:29636-29646 says
sample_pass clones an entry, awaits its terminal screen and dates what it saw, naming that suspension window as the
restart race.","At that revision service/ticker.rs:1292-1424 clones entries, has earlier persistence/observation awaits,
then reads screen_tail and title synchronously before activity observation. hosts/terminal.rs:2229-2247 defines those
synchronous methods.","The immediate-parent-to-reviewed ticker diff directly replaces terminal.screen_tail(sup,
SAMPLE_TAIL_BYTES).await with terminal.screen_tail(SAMPLE_TAIL_BYTES), and removes the awaited title read. The explained
screen-read suspension disappears within the range.","Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
core.rs:29580-29588 retains the inaccurate explanation. Earlier awaits or concurrent worker scheduling still permit old
cloned entries, so this finding does not invalidate the test or prove synchronous code cannot be preempted."] Current
coverage/results: {"SPEC.md":"Searched sampling, activity and late-write terms; no acceptance of the stale
explanation.","SPEC_impl.md":"Terminal switch points at 2140-2141 says screen/title sampling comes from the host
emulator after catch-up. It does not preserve an async capture round-trip contract.","TODO.md
Planned":"Empty.","plans/queue":"No a_late_activity_write, awaited-screen-read or named explanation repair in targeted
searches.","BUGS.md":"Unrelated SSH helper hang only.","INDEX.md and queue":"Read index and searched
awaited/read/late-activity documentation terms; no exact explanation defect. switcher-late-activity-baseline concerns
native GUI input baselines, not this sampler test.","TRIAGE_OUTCOMES.md":"No matching active sampler explanation
decision in targeted searches.","FILTER.md":"Static false lifecycle documentation is not a rare self-correcting
diagnostic; no complete filter match."} Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "Maintainers are given an obsolete async screen-read rationale for retaining a session-wide activity cell.
This has consistently been an internal documentation defect, without product failure, an invalid test oracle, security
impact or user-work loss."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g115 record](records/review-through-00de0b6c/groups/g115.json). Record ID: `l2`.
