# Farhelm M6: multi-host

NOTE: This is the plan for milestone 6 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Turn M1's argv-specified single host into SPEC.md's real topology: a persistent host registry, the helm connected to
every registered supervisor at once, one aggregated session list that survives hosts going dark, and the helm's own
machine always present as a host. This is the milestone where the helm gains its first persistence (helm.db) and its
first connection-state model — and that model is also where two long-standing items land, because PLAN.md's ladder ties
them to it: version-skew refusal surfaced per host, and the terminal websocket's auto-reconnect (found in post-M3 manual
testing: a laptop sleep kills only that socket, and today recovery is a manual back-and-reopen). Real cursor pagination
of the session list replaces M2's hard cap in the same stroke, because multi-host aggregation is what could actually
grow lists past it.

Provisioning stays M7's: a registered destination with no running supervisor is an honest state with a manual-path hint,
never an offer to install.

## User-visible outcome

- A hosts surface in the UI: every registered host with its connection state always visible — connected, actively
  retrying, unreachable with background re-probing, refused for version skew (with both versions named), or awaiting an
  identity decision. Hosts are added by SSH destination, edited in place, and removed; removal forgets the host and its
  cached sessions while the host itself is untouched and reappears on re-registration.
- The machine running the helm is always in the list — "supervisor running" or "not running — start it manually" — never
  a ghost, never needing registration (user decision 2026-08-04).
- One flat session list across every host, each row saying which host it lives on. Sessions on an unreachable host stay
  listed from the helm's last-known knowledge (which survives helm restarts), clearly marked stale; lifecycle operations
  against them are refused with the host's state in the error; opening one shows its metadata behind a host-unreachable
  notice instead of a terminal.
- Session creation gains a host selector, defaulting per SPEC.md: the host of the currently open session, else the
  helm's own host.
- `farhelm helm run` no longer takes its argv session and transport flags — `--ssh`, `--cwd`, `--agent`, `--title`,
  `--remote-farhelm`, `--remote-state-dir`, the last two living on as per-host registry fields (user decision
  2026-08-04: the registry and the create API are the mechanism now). One new flag, `--ensure-hosts <file>`, consumes a
  JSON5 file of hosts the helm guarantees are registered at startup — an additive floor over helm.db, built for
  half-automated setups and agent-driven testing.
- A terminal whose connection dies alone (laptop sleep) reconnects by itself: active retries for about half a minute,
  then occasional background probes with a visible reconnect control the whole time, landing at the tail through M5's
  replay marker like any reattach. A terminal displaced by takeover or detached for a stall never reconnects itself —
  that surface is unchanged.

## Scope

### In

1. **One protocol bump to 8, all M6 wire vocabulary upfront.** (Amended while building item 7: this milestone shipped a
   SECOND bump, to 9, and the rule below is why it was needed rather than a lapse in it — see the note at the end of
   this item.) The standing rule: new tagged-enum variants are connection-fatal to older decoders, so every wire shape
   lands in one proto PR — vocabulary first, handlers later. The vocabulary is small: the supervisor's hello reply gains
   the host identity, `ListSessions` becomes cursor-paginated, and `SessionInfo` gains the creation timestamp it has
   never carried (verified against the wire struct while planning) — the cursor's ordering key, helm.db's ordering
   columns, and the cross-host merge all need it on the wire. Golden and both-direction tolerance tests within 8; the
   version-skew handshake test grows the new boundary.

   Pagination shape: the request carries an optional opaque cursor and an optional page limit; the reply carries the
   page, the total count, and an optional next-cursor whose absence means exhaustion. Ordering is creation-time
   descending with session id as tiebreak — a total order over stable columns, so a cursor is a resume point that
   survives sessions being created or deleted between pages (a new session appears at the front of a later refresh,
   never tears a page mid-walk). The cursor encodes the last-seen ordering key and is opaque to callers. The supervisor
   enforces both the count limit and M2's encoded-size budget per page — a page of fat records shrinks rather than
   oversizing the frame — which retires the truncated flag: truncation-as-error is replaced by
   next-cursor-as-continuation, and the total count keeps the UI's "N of M" honest. M2's cap constant survives only as
   the default page limit. The cursor contract — opacity, ordering, behavior under concurrent mutation — is durable
   protocol semantics, so SPEC_impl.md's transport section records it in the implementing PR rather than leaving this
   plan its sole authority.

   **Bumped again, to 9, by item 7 (2026-08-05).** Item 7's auto-reconnect needs an attach that REFUSES rather than
   displacing another client, and that field — `if_unowned` — is decode-additive but not semantically additive: a
   version-8 peer drops it and performs the displacing attach the caller was trying to avoid, silently, on both ends.
   The vocabulary-first rule above is exactly the argument for bumping (an ignorable safety clause is a wrong answer,
   not a tolerated one), so the second bump is the rule applying rather than the plan being wrong; what was wrong was
   assuming one milestone's wire work is finished when its vocabulary is. The browser edge has no hello to refuse at and
   is gated on the helm's build stamp plus a route only a conforming helm serves; SPEC_impl.md's version-and-skew
   section carries both halves.
2. **Supervisor host identity and pagination serving.** The identity SPEC.md promises ("generated by its supervisor at
   install time") does not exist yet — supervisor_meta holds only the boot id — so this milestone adds it: a UUID
   generated at first run, stored in supervisor_meta via the existing schema-version mechanism, immutable thereafter,
   returned in every hello. Wiping the state dir mints a new identity, which is exactly SPEC.md's reinstall semantics.
   Pagination serving reuses the M2 list path with the new ordering and cursor; tests pin page-walk stability against
   concurrent create and delete.
3. **helm.db.** The helm's first persistence, at `~/.local/state/farhelm/helm.db` per SPEC_impl.md, with a
   schema-version mechanism from day one mirroring supervisor.db's. Two tables carry the milestone: the host registry
   (surrogate row id; a kind — one reserved local row, or ssh with a unique destination; the remote farhelm command and
   remote state dir that argv used to carry, ssh rows only; the host identity once learned, nullable until first
   contact) and the last-known session cache (keyed by registry row — the local row included, which is exactly what lets
   a stopped local supervisor's sessions serve stale like any other host's; caught in review of this plan, whose first
   draft made the local host row-less and thereby uncacheable). The web token and device sessions stay M7's; remembered
   defaults (last-used profile) stay M6.75's with profiles themselves. Atomicity is SQLite's own; nothing here needs
   M3's file-write policy. The settled data model — the reserved local row, uniqueness, cache replacement semantics —
   lands in SPEC_impl.md's helm-internals section in the same PR.
4. **Helm connection manager.** The M1 single-connection transport generalized to one connection actor per registered
   host plus the local one, each owning its ControlMaster/ssh child or unix-socket connection, its reconnect state
   machine, and its slice of the cache. States and cadences (user decision 2026-08-04, "snappy"): on loss or at startup,
   active retries with backoff (1s, 2s, 4s, 8s, 15s, 30s — about a minute) then background re-probes every 45 seconds,
   forever; a host that comes back is noticed within about a minute of returning. The same slow cadence re-probes
   version-skewed hosts (an upgraded host resurfaces by itself) — skew is a distinct state carrying both versions from
   the refused hello plus the remediation (update the host's farhelm binary; SPEC.md demands actionable, not just
   diagnostic), never conflated with unreachable. Identity handling on every successful hello: a first contact records
   the identity; a mismatch against the recorded one freezes the host in an identity-mismatch state carrying both ids
   and connects nothing (SPEC.md: never silently merge) until the user adopts the new identity (which purges the old
   identity's cached sessions — they belong to a dead install) or fixes the destination; an identity already recorded
   under a DIFFERENT registry entry marks this entry a duplicate of that twin: in host management it surfaces as an
   entry needing resolution (edit it or remove it), while the HOST — its state, its sessions — appears exactly once,
   under the twin, which is how SPEC.md's shown-once rule and the user's ability to fix the entry coexist; a duplicate
   entry connects nothing while it stays one. The local host is the reserved registry row, not user management surface:
   auto-created at first startup, no destination, never editable or removable, always present in the list — reached over
   the unix socket at the helm's state dir, probed on the same slow cadence when its supervisor is not running, and
   showing the manual-start hint in that state. The whole manager emits the structured diagnostic trail SPEC.md's
   logging section requires for reconnection — attempts, phase transitions, refusals, identity decisions, recoveries,
   each carrying the host id — which could not exist before this milestone built the thing it describes (SPEC_impl.md's
   logging section says exactly that). The cadence constants and the connection-state taxonomy land in SPEC_impl.md's
   helm-internals section in this same PR — SPEC.md's Errors section already owns the phase contract (bounded retries,
   then periodic re-probing, phase visible); the constants are implementation choices and get recorded where those live,
   so the plan is not the only document that knows them.
5. **Helm aggregation, REST, and the ensure file.** Each connected host's sessions refresh by draining the paginated
   list to exhaustion (bounded pages, never one giant frame) into the cache; the served list is the merge of live hosts'
   latest refresh and stale hosts' cache, each row tagged with its host and a stale flag, ordered by the same
   creation-time key, and paginated at the REST edge with a helm-level cursor over that merged order. The two cursor
   layers are deliberately decoupled — composing per-host wire cursors into the REST cursor would tie one browser page
   fetch to N live host round trips and break whenever a host flapped mid-walk; draining into the cache makes the REST
   cursor a plain resume point over local data. That decoupling is helm architecture with a rationale worth keeping —
   SPEC_impl.md's helm-internals section records it in this PR. REST grows host management (list hosts with state, add
   by destination, edit destination, remove; add always registers and lets the connection state say what was found —
   that is what makes the ensure file meaningful for hosts that are down at startup) and the identity-mismatch
   resolution verb (adopt). Session operation routes keep their `/api/sessions/{id}` shape: ids are UUIDs, the helm
   routes by owner lookup in its merged view, and operations against a session whose host is in ANY non-connected state
   — unreachable, skewed, identity-mismatch, duplicate — are refused with that state named in the error and nothing
   queued; unreachable is not special, it is just the common case. Create takes the target host in the body, and a
   create naming an unreachable, skewed, mismatched, or duplicate host is a precondition failure exactly as SPEC.md's
   creation section already demands — a visible error naming the host's state, and no session. `--ensure-hosts <file>`:
   JSON5, `{ hosts: [ { ssh, remote_farhelm?, remote_state_dir? } ] }`, upserted by destination through the same code
   path as a REST add before serving starts; it never deletes, and after ingestion the file plays no further role. The
   argv session flags (`--ssh`, `--cwd`, `--agent`, `--title`, `--remote-farhelm`, `--remote-state-dir`) are dropped —
   the last two live on as per-host registry fields — and the e2e harness migrates from them to an API-driven create
   (`e2e/start-stack.sh` creates its startup session with a POST once the helm is up). Both are user-facing CLI surface,
   so SPEC_impl.md's CLI section gains the new flag and loses the old ones in this same PR, per that document's standing
   sync rule. README's "Trying it" command flow migrates in this same PR — a README documenting dropped flags would be
   broken for the rest of the stack — while the broader M6 README refresh still closes the milestone.
6. **UI hosts and the multi-host list.** A hosts panel on the list page: per-host state chips always visible (SPEC.md's
   "per-host connection state is always visible"), add/edit/remove with in-page confirmation for remove (wry has no
   native dialogs — established ground), the identity-mismatch decision surfaced where the state chip is, and the local
   host's row with its running/not-running state and manual-start hint. Session rows name their host; stale rows are
   visibly marked; opening a stale session shows metadata behind a notice naming the host's ACTUAL state — SPEC.md's
   host-unreachable notice for the unreachable case, the skew or identity story when that is the real cause, because a
   generic "unreachable" over a skewed host would hide the remediation; refused operations surface the helm's words. The
   create dialog gains the host selector with SPEC.md's default. List filtering stays M6.75's — host names on rows and
   the hosts panel are display, not a filter surface. One more skew edge lands here because SPEC_impl.md names it and no
   milestone ever claimed it: client↔helm load. The UI bundle carries its build stamp and compares it against the helm's
   on every API reply, and a mismatch — a tab left open across a helm upgrade — surfaces a reload prompt rather than
   mysterious failures; the helm↔supervisor hello refusal has existed since M1, and this closes the other edge of
   SPEC_impl.md's version-and-skew section.
7. **UI terminal auto-reconnect.** For transport loss only: the terminal socket closing without a takeover, stall, or
   navigation cause — and transport loss that never surfaces as a close: the laptop-wake case this milestone exists to
   fix classically leaves the socket LOOKING open (a NAT or sleep timeout killed it silently), so waiting for a close
   event would miss the motivating papercut entirely. The terminal path therefore gains the liveness check SPEC.md's
   typing-goes-nowhere rule requires: a heartbeat over the existing WebSocket, idle-gated so an active terminal costs
   nothing extra, whose expiry tears the socket down locally and enters the same visible retry flow as a clean close.
   Active retries with backoff (0.5s, 1s, 2s, 4s, 8s, 15s — about thirty seconds), then background probes every 30
   seconds, with a visible reconnect control from the first failure onward (user decision 2026-08-04). Every successful
   reconnect is an ordinary reattach riding M5's replay-complete marker, so it lands at the tail with no visible
   re-scroll. The carve-outs are absolute: a takeover detach keeps its take-control surface and never auto-reconnects (a
   displaced client bouncing back would fight the new owner — the fight would be visible as the two clients' takeovers
   alternating), and a stall detach keeps its banner (the stalled client's wedge is the reason it was detached;
   reconnecting into the same wedge helps nobody — the user acts first). The reconnect phase is visible in the terminal
   surface, distinct from M5's connecting placeholder. The transport-loss-only rule and the carve-outs are durable
   product behavior no document states yet — SPEC.md's Errors section assigns reconnection to this milestone's
   connection-state model but never scopes which detaches reconnect — so SPEC.md's terminal section records them in this
   same PR, exactly as M2.5 landed the stall-detach contract and M5 the rename bounds: the spec must not stay silently
   narrower than the implementation.

### Out (deliberately)

Provisioning, web-token auth, spawn, archive, Mac packaging (M7). Status heuristics, profiles, list filtering, live push
— the M2 list poll and M4 detail poll remain the freshness mechanism (M6.75). Profile syncing (post-v1). Host display
names — SPEC.md's registry entries are SSH destinations, and M6 invents no naming surface. Queued operations against
unreachable hosts — SPEC.md v1 refuses, nothing queues. Cache-serving for REACHABLE hosts' terminal or detail data — the
cache exists for the stale list, not as a general layer. Any auto-reconnect for takeover or stall detaches. Deleting a
stale session's cache entry individually — disposal is by removing the host, per SPEC.md.

## Testing decisions (settled while planning)

Proto changes get the same golden and tolerance coverage every bump has gotten, plus the version-skew boundary — at 8
when this was written, and at 9 once item 7's non-displacing attach earned the second bump, with the boundary test
following the constant rather than a literal. Pagination is pinned at the supervisor integration level: full-walk
equivalence with the unpaginated truth, page stability across interleaved create/delete, the encoded-size budget
shrinking a fat page, and cursor opacity (a tampered cursor is a clean error, not a panic or a wrong page).

The connection manager is tested against scripted supervisor peers for the state machine (loss mid-list, hello refusal
for skew, identity change, duplicate identity, cadence transitions under a test clock) — and against one real
ssh-to-localhost round trip for the transport truth, with an isolated remote state dir on the registry entry (the field
that replaces `--remote-state-dir`) so the "remote" supervisor on this same machine never touches the user's real state
dir. The ssh test skips loudly where passwordless `ssh localhost` is absent, exactly like the cgroup tests skip without
a user manager; CI provisions self-ssh (keygen, authorized_keys, sshd) in the workflow so the loud skip never hides the
path there. Cache semantics are pinned across a helm restart: kill the helm, restart against a now-unreachable host, and
the stale list must serve from helm.db — for an ssh host AND for the local one (its reserved row exists exactly so this
test can pass; a local-only variant guards it). The restart leg runs WITHOUT the ensure file, deliberately: an ensure
file at restart would rebuild the registry entry and mask a broken persistence path, and the assertion is that
destination, identity, and stale sessions all come from helm.db alone. Create against a down host is pinned at the REST
level: visible error naming the host's state, no session anywhere.

Playwright covers the user-visible contracts end to end with two real supervisors — the local one plus an
ssh-to-localhost "remote" — behind named tests the UI PRs cite: hosts-panel-states, add-host-discovers,
unreachable-host-goes-stale, stale-session-metadata-view, op-refused-on-unreachable, identity-mismatch-surfaced,
local-host-always-listed, create-dialog-host-selector, and for reconnect: socket-killed-reconnects-and-lands-at-tail
(kill the socket server-side, assert the reattach rides the marker and the terminal never shows a re-scroll),
takeover-does-not-bounce-back, retry-exhaustion-shows-reprobe-phase, create-on-unreachable-refused,
wedged-socket-detected (hold the socket open server-side while silencing it — the heartbeat expiry must enter the same
retry flow a close does), and remove-and-re-add-host (removal forgets the host and its cache while the supervisor and
its sessions run on untouched; re-adding the destination rediscovers everything — SPEC.md's remove-merely-forgets
contract, executable). The ensure file is exercised in the harness itself: `start-stack.sh` grows the second supervisor
and registers the ssh "remote" through `--ensure-hosts` — the local host needs no registering — which makes the harness
the feature's first consumer.

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md header update it implies.
2. Proto: the complete M6 wire vocabulary, one bump to 8. (Item 7 later adds the non-displacing attach and bumps to 9 —
   see item 1's amendment.)
3. Supervisor: host identity and pagination serving, with their integration tests.
4. Helm: helm.db — schema, registry and cache storage, migration mechanism. Storage only; no behavior change.
5. Helm: the connection manager — per-host actors, state machine, cadences, identity handling, local-host synthesis.
6. Helm: aggregation, REST host management, session-op routing, the ensure file, the argv-flag drop, and the e2e-harness
   and README-command migrations the drop forces. REST/WS contracts freeze here.
7. UI: hosts panel, multi-host list, create-dialog host selector, Playwright coverage.
8. UI: terminal auto-reconnect, Playwright coverage, and the README refresh to M6 (the milestone-closing PR).

## Acceptance

M6 is done when all of the following hold, pinned by automated tests:

1. Two hosts — the local supervisor and an ssh-to-localhost remote with an isolated state dir — serve one merged session
   list; each row names its host; sessions open, stop, and delete on both through the same UI.
2. Killing the remote's supervisor flips its host chip through active-retry into unreachable-reprobing; its sessions
   stay listed, marked stale; operations on them are refused with the host's state named; opening one shows metadata
   behind the host-unreachable notice; restarting the supervisor resurfaces the host within about a minute without any
   user action.
3. The stale list survives a helm restart: with the remote still down, a freshly started helm lists its sessions from
   helm.db, marked stale.
4. A host whose supervisor presents a different protocol version shows version-skew with both versions named, and
   recovers by itself on the re-probe cadence once versions agree.
5. An identity change at a known destination freezes the host in identity-mismatch until the user adopts (old cache
   purged) or fixes the destination; a second destination reaching an already-registered identity surfaces as an entry
   needing resolution while the HOST list shows exactly one host — the assertion counts displayed hosts, not just
   labels.
6. The local host is always listed; with no local supervisor running it says so with the manual-start hint and offers no
   phantom sessions.
7. `--ensure-hosts` registers a JSON5 file's hosts at startup through the add path, additively and idempotently; the
   argv session flags are gone; the e2e harness stands its stack up through the ensure file and the create API.
8. The session list is cursor-paginated end to end — supervisor pages under the size budget, helm serves a stable merged
   cursor — and a list wider than one page walks completely and in order, with the total count shown.
9. A terminal socket killed mid-session reconnects unaided within the active window and lands at the tail through the
   replay marker with no visible re-scroll; a socket wedged open-but-silent is detected by the heartbeat and recovers
   through the same flow; a takeover's displaced terminal and a stall-detached terminal never auto-reconnect; the manual
   reconnect control works in every reconnect state.
10. The full CI gate is green on every PR (with CI's new self-ssh provisioning in place).

## Risks retired by this milestone

- The helm stops being a single-connection proxy: the connection actor, per-host state machine, and cache are the
  substrate M6.75's push channel and M7's provisioning both assume, built against their real multi-host shape.
- The stale-list promise — SPEC.md's "sessions on an unreachable host stay in the list, clearly marked" — becomes
  mechanical, with its persistence proven across helm restarts.
- Identity semantics (stable across address changes, never silently merged, duplicates shown once) meet reality before
  provisioning automates host setup on top of them.
- The M2 list cap dies before multi-host aggregation could have made it a real ceiling, and the frame-size discipline
  gets its second bulk consumer after M4's uploads.
- The laptop-sleep reconnect papercut — the last manual recovery in daily use — closes with the designed
  connection-state behavior instead of a client-side quick fix that would have preempted it.
