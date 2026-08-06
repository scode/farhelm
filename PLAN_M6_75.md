# Farhelm M6.75: status and profiles

NOTE: This is the plan for milestone 6.75 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Give sessions their real statuses — running, waiting, idle — and give agents their profiles, and make both arrive in
every client without polling. The three land together deliberately: status transitions are what make polling genuinely
painful (PLAN.md moved this milestone after multi-host so the push channel gets designed ONCE against its real shape),
and profiles are what make status sharpening per agent kind a user-visible choice rather than a hardcoded pair of
regexes. The milestone replaces ALL FOUR of the UI's periodic loops — M2's session-list poll, M4's
session-detail/tab-list poll, and the two host readers that grew beside them (the list page's hosts poll and the session
view's host-state poll) — with one live invalidation feed, adds list filtering and search across the dimensions SPEC.md
names, and ships profile CRUD with editable starter profiles.

## User-visible outcome

- Every session shows running, waiting, or idle while alive — running while the agent works, waiting when a detected
  question or approval sits unanswered, idle at rest — alongside the existing exited/interrupted/error statuses. Wrong
  status is cosmetic only; nothing about interaction ever waits on it.
- Changes appear everywhere by themselves: a create, rename, stop, delete, status flip, or host connectivity change
  shows up in every open client without refresh, list page and session view alike.
- The session list gains filtering and search by host, directory, agent profile, status, and title.
- Profiles become a managed thing: create, edit, delete named agent definitions per host, with editable starter profiles
  for Claude Code and Codex on every fresh supervisor; session creation offers the target host's profiles and defaults
  to the last-used one, asking instead of guessing when that profile is gone.

## Scope

### In

1. **Supervisor periodic ticker.** The supervisor gains its first internal cadence: a `service/ticker.rs` task started
   by `serve`, holding a `Weak<Supervisor>`, driving (a) the new status sample pass and (b) the conversation-capture
   sweep. The helm's 3-second `ListSessions` drain REMAINS (item 3) — this is decoupling, not preservation-after-
   removal: capture today advances only because an external caller happens to poll, which is a cadence the supervisor
   does not own and no contract guarantees; a supervisor responsible for its own capture and sampling stops being
   hostage to whoever dials in. Sampling must never sit on the attach/input path (SPEC.md's status rule), and the ticker
   respects `may_record` and the handler-admission bounds around pane_states. Shutdown seam included so tests can start
   and stop it deterministically.

2. **Status classification and per-kind sharpening.** The baseline classifier lives in `service/status.rs` beside the
   existing precedence rules it extends: alive sessions classify from sampled pane activity (output recency from the
   ticker's samples, captured tail shape), below `Error` and dead-pane outcomes in the existing precedence. The
   waiting/idle boundary is heuristic by contract; the generic baseline is activity-based (recent output = running,
   quiet = idle), and per-kind sharpening is where waiting detection gets real: `AgentIntegration` gains
   `fn sharpen(&self, baseline, tail) -> SessionStatus` as a DEFAULTED method (the `Option`-shaped `integration_for`
   means "no sharpening", never "no status" — the audit that shaped this is recorded in the assessment notes), with
   Claude Code and Codex implementations recognizing their prompt/approval shapes in the captured tail. `agent_kind.rs`
   splits into a directory (`mod.rs` + `capture.rs`) when the regexes land — and the module doc gets REWRITTEN for the
   three-part subsystem it becomes (kind seam, capture, sharpening); its current "two halves, deliberately one module"
   argument is specifically an argument against this split and must not be transplanted unchanged. Fixture-driven unit
   tests per kind; wrong-status-is-cosmetic pinned by never letting classification touch the attach path (the ticker
   seam makes that structural).

3. **Proto v10 — all M6.75 vocabulary in one PR.** The standing rule: wire shapes never trickle. The bump carries: the
   `running`/`waiting`/`idle` statuses (SessionStatus's live split — every consumer of `Alive` migrates; the existing
   `Unknown` becomes internal/compat vocabulary that must never RENDER: for a restart the helm's never-overwrite-
   definite rule already holds the prior status, and for a fresh create — where no prior definite exists — the UI shows
   no status badge until the first classified status lands. No latency is promised for that gap: the post-write wake
   removes the CADENCE wait but bounds nothing (the drain still does real work, and the ticker's first sample has no
   ordering against it) — the honest bound is one ticker interval plus one drain, and the badge's absence is what makes
   the gap harmless at any length. A create test and a restart test prove no unknown badge ever paints, separately,
   because the two mechanisms differ), profile vocabulary (profile records on the wire; CRUD request/reply pairs;
   `CreateSession` gaining a profile-backed MODE that is mutually exclusive with the raw-invocation mode — a request
   naming both is rejected, the selected mode and profile identity join the create-idempotency fingerprint so a retried
   create cannot flip modes, and the raw mode stays for the e2e harness and tests — NOT for the ensure-file, which
   registers hosts and never creates sessions), and a durable SOURCE-PROFILE snapshot on sessions: an OPTIONAL wire
   field (absent = raw-created) carrying the immutable profile id and the name as snapshotted — nothing mutable lives in
   the snapshot; the profile's CURRENT existence is DERIVED when a reply is built, by one catalog lookup on the id
   (absent from the catalog = deleted; present under another name = renamed), so historical sessions never get rewritten
   on a delete and there is exactly one copy of existence truth (a deleted profile's sessions still filter under their
   snapshotted name, rendered as no-longer-existing). Remembered defaults need NO wire plumbing — the helm owns them in
   helm.db and resolves the default before sending a concrete profile selection. Golden tests, both-direction tolerance
   tests, version-skew boundary at 10. DECIDED here so the proto PR is not re-litigated: the helm↔supervisor edge keeps
   its 3-second drain cadence plus the existing post-write wake — no wire push in v10. The push problem this milestone
   solves is the CLIENT edge; the supervisor edge's drain imposes an accepted staleness bound of one drain interval
   (~3s) on status freshness, already matched to the old UI poll cadence, and a supervisor-side push channel would
   duplicate exactly the coalescing the helm must build anyway. If dogfooding shows that bound is painful, that is
   M7-era evidence and a deliberate later bump.

4. **Supervisor profiles.** The named catalog and CRUD are greenfield, but the SNAPSHOT half already exists and gets
   reused, not rebuilt: sessions already carry durable `agent_kind`/`resume_template` snapshot columns and the immutable
   `IntegrationSnapshot` seam — creation-by-profile resolves the profile and feeds exactly that existing seam, adding
   the source-profile identity (item 3) beside it. New: a `profiles` table via the schema ladder, CRUD handlers, starter
   profiles for Claude Code and Codex seeded at first run (editable, deletable — SPEC.md's "a fresh supervisor is not
   empty"), SPEC.md's snapshot rule across edit and delete, and the unknown-profile precondition: a create naming a
   profile that no longer exists fails visibly with no session, checked before launch (SPEC.md's creation-failure split;
   a profile can vanish between the picker read and the submit, so this is a real race, and it must surface — never
   silently fall back to another profile). A profile names its integration kind from the built-in catalog or none
   (generic). The existing `agent_kind` override and raw-invocation path stay — profiles layer over them, and the e2e
   harness keeps creating sessions the raw way where a profile would be ceremony.

5. **Helm fleet events, server-side filtering, and the invalidation feed.** The helm learns to say "something changed":
   a coalescing revision counter (`FleetEvents`, a `watch`-shaped bump-and-reread, matching how `aggregate` already
   reasons) owned by the ConnectionManager, published from its existing chokepoints — actor state transitions, committed
   session-cache CHANGES (a refresh whose replace writes an identical row set publishes nothing; the write path knows
   whether it changed anything, and no-op refreshes must not wake every client every 3 seconds), the identity-less
   in-memory refresh path under the same changed-only rule, registry shape changes, AND successful profile mutations and
   remembered-default writes — the goal promises profiles arrive without polling too, so a profile edit in one client
   must invalidate another client's open profile surface and create dialog. The feed endpoint itself carries NO data: it
   is a WebSocket that delivers revision notifications only, and clients re-read whatever their current surface needs
   through their readers — the existing paginated list, detail fetch, and hosts read, plus item 4's NEW profiles read
   where that surface is mounted. That keeps one payload shape, makes lagged subscribers collapse trivially to one
   re-read, and reuses every consistency rule the REST layer already enforces. Filtering moves SERVER-SIDE — and
   specifically into the HELM's merged-view query, nowhere else: the drains keep pulling each supervisor's COMPLETE list
   (a filtered drain would corrupt the cache's whole-fleet completeness, and host filters, identity-less sessions, and
   stale retained rows only exist merged), predicates apply before the page cut (a client-side page filter would make "N
   matching" incoherent with rows beyond the page), and the REST list gains the filter parameters plus a SECOND count:
   the reply carries both the matching total and the overall fleet total, because "N matching of M sessions" needs both
   numbers and today's shape has one — truncation cuts the MATCHING walk, while the incoherence check stays against the
   overall count it has always described. The supervisor's `ListQuery` remains cursor-and-limit only — no supervisor-
   side consumer needs filtering (its one production caller drains complete lists), and it grows nothing until one does.
   Profile CRUD proxying to the owning supervisor rides the existing request path (`host_client`); the helm remembers
   the last-used profile per host in helm.db (the remembered-defaults table SPEC_impl already assigns there) and serves
   it to CLIENTS through the profiles response — "no wire plumbing" means the SUPERVISOR protocol: the browser still
   needs the helm-owned default, so the per-host profiles read returns the catalog plus the remembered default id in one
   shape.

6. **UI: the feed replacing all four periodic loops.** A `feed` module owning the socket subscription and a
   `GlobalSignal` revision, mounted at App level beside the skew notice — the channel must survive navigation between
   the two mutually exclusive pages, which is exactly why neither page can own it. The socket itself is a small separate
   JS asset (`events.js`) following the established island pattern (Rust owns policy, JS owns the socket). On each
   revision bump, the mounted page re-reads through its existing commit path: `commit_listing` and the hosts reader on
   the list page, `commit_detail` and the host-state reader in the session view (the commit closures — the seams cut by
   the assessment refactors that ran alongside M6.5 — become the feed's consumers, preserving every reconciliation rule
   they carry). All four poll `use_future`s die. Fallback discipline, per SPEC_impl's withdrawal rule: while build
   stamps MATCH and only the feed is unhealthy (socket down, reconnecting), the documented poll fallback runs; under
   build SKEW, feed and fallback polling BOTH stop — polling data that carries this milestone's vocabulary is exactly
   the unattended behavior the skew gate exists to withdraw, and the reload prompt is the way forward. The
   fallback-to-feed handoff is pinned against its race: on (re)subscription the server sends the CURRENT revision
   immediately, and the client performs one re-read on receiving it BEFORE disabling the fallback poll, so a mutation
   landing between the last poll and the subscription cannot go unseen.

7. **UI: status column, filtering, and search.** The status column renders the new statuses through the existing
   `status.rs` badge seam. Filtering and search are a QUERY surface, not a render pass — the UI builds the filter
   parameters (host, directory, profile, status, title; the parent-reference filter SPEC.md ties to spawned sessions is
   explicitly deferred to M7 WITH spawn, recorded in Out) and the helm's server-side filtering from item 5 answers;
   `rows.rs` keeps its existing overlay/banner derivations, and the count banner distinguishes "N matching of M" from
   truncation using the seam extracted for it. Named Playwright tests per contract, including the two deterministic
   route-controlled tests deferred here from M6.5's review (the stop-refetch-versus-poll ordering and the restart-epoch
   staleness guard — they pin the commit closures the feed now drives), plus the incoherent-banner browser case and the
   PeerLine render-shape pin, all deferred to this milestone by name.

8. **UI: profile management.** Profile CRUD per host (the hosts surface grows a profiles section), the create dialog's
   profile selector defaulting to last-used with SPEC.md's ask-don't-guess fallback, and the snapshot rule surfaced (an
   edited profile visibly does not touch existing sessions). Named Playwright tests.

### Out (deliberately)

Notifications (non-goal). Automatic initial-prompt delivery (post-v1; the profile schema deliberately does not grow a
prompt field). Profile syncing across hosts (post-v1). Supervisor-edge wire push (decided against in item 3). Archive,
spawn, auth (M7) — and with spawn, the parent-reference filter SPEC.md attaches to agent-spawned sessions: the filter
ships in M7 beside the feature that mints parent references, not here where no session has one. List-filter persistence
across sessions (nothing in SPEC.md asks for it). Any status integration requiring agent configuration (SPEC.md forbids
it).

## Testing decisions (settled while planning)

Status classification is pinned at three levels: pure unit tests over the classifier and each kind's sharpen (fixture
tails, no tmux), ticker integration tests against real tmux with the injectable cadence (a busy pane classifies running;
a quiet one decays to idle; a fixture tail with a Claude approval prompt classifies waiting), and one Playwright test
asserting a status transition arrives via the feed without refresh. The fake agent grows fixture support where needed (a
script that emits a waiting-shaped tail on cue) rather than any real-agent dependency.

The feed is pinned by the deferred-from-M6.5 route-controlled tests plus: a two-client Playwright test (change from one
browser context, assert the other sees it without refresh — SPEC.md's multi-client sentence, executable), a no-polling
proof (route/request observation asserting that a healthy feed performs NO periodic reads — the two-client test alone
cannot distinguish push from a surviving poll), a feed-death test (kill the event socket; the client falls back to its
documented poll and recovers the feed per the reconnect discipline), a mutation-during-reconnect test (a change landing
between the fallback's last poll and the resubscription is seen — pinning the send-current-revision handshake, which the
lagged-subscriber test does not cover for a NEW subscription), a skew test (mismatched build stamps stop feed AND
fallback polling both), and helm-level tests driving FleetEvents through scripted supervisors (a status flip bumps the
revision; a NO-OP refresh does not; a lagged subscriber re-reads cleanly; identity-less hosts publish through the same
changed-only rule).

Profiles are pinned by supervisor integration tests (CRUD, starter seeding, snapshot and tombstone persistence across
edit and delete, creation-mode exclusivity and its idempotency fingerprint, the unknown-profile precondition failing
before launch with no session) and helm-level tests for proxying, remembered-default resolution, and filter-by-profile
surviving edit/rename/delete — the split matters: the supervisor owns catalog and snapshot truth, the helm owns
defaulting and the merged-view filter — plus Playwright coverage of the CRUD surface, the ask-don't-guess create
fallback, and a two-client profile test (an edit in one client invalidates the other's open profile surface via the
feed).

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md header update it implies (M6.5 moves to history).
2. Proto v10 — the complete vocabulary, one bump.
3. Supervisor ticker (capture decouples from the ListSessions carrier; sampling infrastructure).
4. Supervisor status classification and per-kind sharpening.
5. Supervisor profiles (schema, CRUD, starters, snapshot-at-creation, creation modes).
6. Helm FleetEvents + the invalidation endpoint + merged-view filtering + profile proxying + remembered defaults.
7. UI feed replacing all four loops (listing, hosts, detail, host-state); status column, filter/search query surface.
8. UI profile management, and the milestone-closing README refresh.

Steps 7 and 8 may run as parallel tracks once step 6 freezes the contracts, per the goal's standing recipe; the split
point is the feed (7 owns it, 8 consumes it), so 8 assembles after 7 lands.

## Acceptance

M6.75 is done when all of the following hold, pinned by automated tests:

1. A session's status transitions running → idle → waiting are produced by the sampler and sharpeners, arrive in an open
   client without refresh, and never gate interaction (typing into a mis-classified session works untouched).
2. All four periodic loops are gone — listing, hosts, detail, host-state — and the pages update via the feed alone,
   proven by the two-client test AND the no-polling request observation; feed-death (stamps matching) falls back to the
   documented poll; build skew stops feed and fallback both.
3. Profile CRUD round-trips through helm to supervisor; starter profiles exist on a fresh supervisor; the snapshot rule
   holds across profile edit and delete; an unknown profile fails the create visibly with no session; creation defaults
   to last-used and asks when it is gone.
4. The list filters and searches by host, directory, profile, status, and title — server-side, coherent with pagination
   and totals — including sessions whose source profile was since edited or deleted, with the count banner honest about
   filter-versus-truncation.
5. The deferred M6.5 test debts land: stop-refetch ordering, restart-epoch guard, incoherent banner, PeerLine render
   shape.
6. The full CI gate is green.

## Risks retired by this milestone

- The push channel is built once, against multi-host reality — aggregation, stale transitions, pagination all exist —
  instead of single-host and reworked (the reordering's whole argument, now cashed in).
- The capture sweep stops depending on an external cadence nobody promised it — the drain remains, but the supervisor no
  longer needs it.
- Status lands with the seams the assessment refactors cut for it alongside M6.5 (status.rs both sides, predicates,
  commit closures), so the tripling of live statuses is arms in one module, not a fan-out hunt.
- Profiles arrive before M7's spawn needs `--agent` resolution against them, and before provisioning makes "a fresh
  supervisor is not empty" a user-facing promise.
