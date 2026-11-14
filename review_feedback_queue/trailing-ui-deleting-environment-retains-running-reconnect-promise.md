# Deleting environment can promise its cached session is still running

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: a retained session's unavailable card can promise that the session keeps running and
will reconnect automatically while another window destroys its environment. Confirmed source interval: window A views an
offline environment's cached session; sessions.rs:3162-3185 serves that detail with stale true. Window B performs
supported whole-environment Delete while the supervisor is disconnected. Helm hosts.rs:1513-1548 publishes a connection
hold with HostState::Deleting { failure: None } and awaits provider.destroy; delete_registered_host and cache purge
follow provider success at 1560-1564. The hold retains the actor, so the session and its host remain available to window
A during provider destruction. manager.snapshots:2183-2187 exposes Deleting. aggregate.rs:541-625 reads every
cache-serving snapshot, including Deleting, and row_of:357 sets stale from !state.is_connected; it does not filter away
these cached rows during deletion. This confirms an actual retained stale-session interval, not merely a formatter with
no consumer. Window A has no own delete request, so page-local header_deleting does not conceal its card. UI
session_view.rs:1938-1942 resolves the owned host from sidebar_hosts; 2020-2023 calls
sandbox_sessions::unavailable_notice for shown.stale. That wrapper:104-137 overrides Suspended and Starting only.
hosts::stale_session_notice:1757-1764 therefore uses the generic explanation 'is not answering. The session keeps
running there; this view reattaches by itself', because Deleting without failure has no remedy.
SessionUnavailableCard:3363 renders that explanation b

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g031/f4`.

### g031/f4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998; synthesis checked through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: a retained session's unavailable card can promise that the session keeps
running and will reconnect automatically while another window destroys its environment. Confirmed source interval:
window A views an offline environment's cached session; sessions.rs:3162-3185 serves that detail with stale true. Window
B performs supported whole-environment Delete while the supervisor is disconnected. Helm hosts.rs:1513-1548 publishes a
connection hold with HostState::Deleting { failure: None } and awaits provider.destroy; delete_registered_host and cache
purge follow provider success at 1560-1564. The hold retains the actor, so the session and its host remain available to
window A during provider destruction. manager.snapshots:2183-2187 exposes Deleting. aggregate.rs:541-625 reads every
cache-serving snapshot, including Deleting, and row_of:357 sets stale from !state.is_connected; it does not filter away
these cached rows during deletion. This confirms an actual retained stale-session interval, not merely a formatter with
no consumer. Window A has no own delete request, so page-local header_deleting does not conceal its card. UI
session_view.rs:1938-1942 resolves the owned host from sidebar_hosts; 2020-2023 calls
sandbox_sessions::unavailable_notice for shown.stale. That wrapper:104-137 overrides Suspended and Starting only.
hosts::stale_session_notice:1757-1764 therefore uses the generic explanation 'is not answering. The session keeps
running there; this view reattaches by itself', because Deleting without failure has no remedy.
SessionUnavailableCard:3363 renders that explanation beside a correct Host being deleted cause and destruction detail.
Contract: SPEC_impl.md Environment Delete and the seal:6647-6649, 'While a host is Deleting, its sessions show as being
deleted with their actions unavailable.' Suggested bucket: other for contradictory diagnostics/false reassurance; no
additional loss-of-work route established. Confidence: confirmed by source and caller sequencing, no execution.
proposed_drop: false. possible_cover: none. Could-be-highest: no established route. Later-fix check: git diff from the
reviewed commit through tip is empty for helm hosts.rs and sessions/sandbox.rs. Follow-up
2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa adds Deleting to frozen/being_deleted and disables actions, but leaves
unavailable_notice's explanation unchanged. Tip session_view still builds the card from shown.stale || sandbox_frozen
and displays its unchanged explanation; new metadata labels and action help do not replace this card. Original
consequence retained. The concrete offline whole-environment deletion interval supersedes reliance on a narrower
seal/cache-refresh race, without asserting that any sealed supervisor still has a live session. No unverified premise
remains for the retained interval.

Returned proposal (claims-018, event 15): **retain-verified**, suggested bucket **other**.

During supported whole-environment destruction, a retained cached session can receive the generic promise that it keeps
running and will reconnect, beside correct deletion details.

Original stronger consequence: False reassurance that a cached session keeps running and reconnects while its
environment is being destroyed. No additional unauthorized destruction or user-work-loss route was alleged. Earlier
seal/cache race reasoning was replaced with the supported offline whole-environment interval.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain contradictory diagnostics for the retained-view interval. Do not claim a sealed supervisor still
has a live session or that action disabling remains unfixed.

Existing parent disposition (event 15): verified. Parent explicitly accepted named retain-verified proposal from
claims-018 after inspecting it. During supported whole-environment destruction, a retained cached session can receive
the generic promise that it keeps running and will reconnect, beside correct deletion details. Retain contradictory
diagnostics for the retained-view interval. Do not claim a sealed supervisor still has a live session or that action
disabling remains unfixed. Checked source/results:
["222bf6253bcd168db04bcbb1df20180c88306998:crates/farhelm-helm/src/hosts.rs:1513-1564 holds the actor in Deleting before
awaiting provider.destroy; delete_registered_host follows successful destruction. Cached data is retained during that
await.","222bf625:sessions.rs:3162-3185 serves cached session detail with stale true when status.client is absent. UI
session_view.rs:1938-1942 resolves its owned host, :2020-2032 obtains unavailable_notice for the stale/frozen view, and
:3363 displays its explanation.","222bf625:UI hosts.rs:1757-1764 gives Deleting without failure the generic
keeps-running/reattaches explanation; sandbox_sessions::unavailable_notice overrides Suspended and Starting only.","Tip
00de0b6c:sandbox_sessions.rs:209-237 still overrides only those two states, while frozen includes Deleting at :93-102.
The later action-gating fix therefore does not repair the explanation. git diff 222bf625..00de0b6c for helm hosts.rs is
empty. Source only; the two-window retained interval was not executed."] Current coverage/results: ["SPEC.md Sandbox
sessions:1524-1540 and Container sessions:4839-4849 require visible deletion/retry; SPEC_impl.md:6150-6158 says Deleting
sessions show as being deleted. No acceptance of a concurrent running/reconnect promise.","TODO.md Planned is empty;
plans/queue/INDEX.md has no unlanded entries. BUGS.md is unrelated.","INDEX.md and queue searches for
deletion-card/running promises found no exact item. sandbox-outage-card-ignores-host-refresh-failure.md was read in
full: it concerns stale Suspended after failed host refresh and successful Resume, not a fresh Deleting snapshot during
destruction. Active TRIAGE_OUTCOMES.md has no exact decision.","FILTER.md is not exact coverage: ordinary offline
whole-environment Delete is sufficient, rather than a rare interruption/timing trigger."] Exact cited cover: [] Original
stronger consequence and missing premise remain preserved: "False reassurance that a cached session keeps running and
reconnects while its environment is being destroyed. No additional unauthorized destruction or user-work-loss route was
alleged. Earlier seal/cache race reasoning was replaced with the supported offline whole-environment interval."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g031 record](records/review-through-00de0b6c/groups/g031.json). Record ID: `f4`.
