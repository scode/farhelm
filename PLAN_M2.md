# Farhelm M2: sessions as a managed thing

NOTE: This is the plan for milestone 2 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Turn M1's single hardwired session into sessions as a managed thing: many at once, a real list, create/open/stop/delete
from the GUI, and metadata that survives the supervisor process. This is where dogfooding starts — the milestone is done
when Farhelm is the tool actually used to run several agents on this host, not a demo driven over curl.

The supervisor already handles multiple sessions (M1's create API never assumed one); what is missing is everything
around that fact: a UI that shows more than `list.first()`, lifecycle operations beyond create, and a place for session
metadata to live that is not the supervisor's process memory.

## User-visible outcome

- Opening the web UI shows a flat session list: title, working directory, invocation, and a truthful status — alive, or
  exited with the exit code when known. The list polls, so changes made from any client show up within seconds.
- Sessions are created from a dialog (working directory, agent command, optional title) that reports precondition
  failures visibly — a bad directory fails the create with the supervisor's actual error, and no session appears.
- Clicking a session opens its terminal, exactly as in M1; a back control returns to the list.
- Stop terminates the agent and its entire process tree; the session stays in the list, terminal still viewable. Delete
  removes the session and its stored state entirely, confirming first when the agent is still alive. Stop does not
  confirm — SPEC.md gives confirmation to delete and archive, not stop, and stop is the recoverable operation.
- After a supervisor restart, sessions are still listed (from SQLite) rather than vanishing. When the private tmux
  server survived the restart, its sessions simply keep working — the terminal handles reload and attach behaves as if
  nothing happened; refusing that would mean reporting a running agent as exited, which SPEC.md's no-guessing rule
  forbids. When tmux did not survive, the session lists as exited with unknown exit code, and opening it shows metadata
  plus why there is no terminal. That second half is deliberately crude — M3's interrupted classification replaces it.

## Scope

### In

- **Supervisor SQLite** (rusqlite, `supervisor.db` in the state dir per SPEC_impl.md): session rows — id, title, cwd,
  invocation, creation time — plus an explicit schema-version mechanism from day one so M3+ migrations have somewhere to
  stand (SQLite's `user_version` pragma or a version table; the implementing PR decides and records why). tmux remains
  the truth for liveness; the DB is the truth for metadata and for the fact that a session exists.
- **Proto growth**: stop and delete control messages; status (alive/exited + exit code) in list replies; the list reply
  capped (~500 sessions) with a total count and a truncated flag. A count cap alone does not bound encoded bytes —
  session records are variable-length — so the reply must also respect an encoded-size budget under the frame limit,
  truncating further (flag still set, count still total) when records are fat; M1's oversize-error defusal stays as the
  last-resort backstop. One PROTOCOL_VERSION bump in the first PR that touches the wire; every later M2 wire change must
  be strictly additive and tolerant on decode (unknown fields ignored, absent fields defaulted) so builds within the
  bumped version interoperate — anything that cannot be additive gets its own bump.
- **Process-tree stop**: SPEC.md's contract — the agent and every descendant (MCP servers, dev servers) die; terminal
  tabs do not exist yet so the rest of the session is just its terminal, which stays viewable.
- **Helm API**: stop and delete endpoints, status/count/truncation passed through in the session list, and widening of
  the protocol error taxonomy only as the GUI's error surfacing demands it.
- **UI**: the list view replacing M1's first-session view; navigation between list and terminal; the create dialog; stop
  and delete actions, delete confirming per SPEC.md. Polling for list freshness. When the list reply is truncated, the
  UI says so (shown N of M) rather than presenting a silently incomplete list — the count and flag exist to be
  displayed, not just plumbed.
- **Tests in the same PR as the behavior**: Rust integration tests for persistence, stop/delete, restart-gap listing,
  and the list cap; Playwright for the UI including a multi-session flow (create two, stop one, delete one, list
  reflects all of it).
- **CI desktop check** (M1 debt riding along): a job that builds the never-yet-exercised desktop feature so it cannot
  rot silently.

### Out (deliberately)

Rename, restart, and archive (M3+, where resume and durability live); running/waiting/idle status heuristics (M5);
interrupted/error classification and supervisor-restart rediscovery (M3); live push of list changes (M5 — polling is the
M2 mechanism); any helm-side persistence (M6 owns the last-known session cache); cursor pagination of the list (deferred
with the cap standing in); profiles (M5 — argv is still the profile); multi-host (M6); attachments and terminal tabs
(M4); web auth (M7).

## Order of work

Each step leaves something runnable; later steps only add.

1. This plan plus the PLAN.md ladder updates it implies.
2. CI desktop-feature build check.
3. Supervisor SQLite: sessions persist; restart gap behaves as specified above.
4. Stop and delete through supervisor and proto, with the version bump.
5. List status, cap, count, truncation.
6. Helm API surface for all of it.
7. UI session list and navigation.
8. UI create dialog and stop/delete actions.

## Acceptance

M2 is done when all of the following hold:

1. Two or more sessions created from the GUI run real agents side by side; both appear in the list with truthful status;
   each opens to its own live terminal.
2. Stop from the GUI kills the agent's whole process tree — pinned by a test whose fake agent spawns a child that must
   also die — and the session remains listed and viewable.
3. Delete from the GUI removes the session and its stored state in any state, confirming first when the agent is alive.
4. After `farhelm supervisor run` is killed and restarted, previously created sessions are still listed: still
   attachable when the tmux server survived, exited-unknown (metadata plus an explanation instead of a terminal) when it
   did not.
5. The list reply is capped with total count and truncated flag, never exceeds the frame limit even with fat records,
   and the UI visibly indicates truncation — pinned at the protocol level and in a UI test.
6. `cargo test` and the Playwright suite cover the above and pass in CI, including the multi-session flow.
7. CI builds the desktop feature.

## Risks this milestone retires

Two sources of truth (SQLite for metadata, tmux for liveness) and the drift discipline between them; process-tree
termination completeness on a real host; the UI growing from one hardwired view to navigation without adopting a router
framework prematurely; whether polling is livable ergonomics for a several-session dogfooding workflow (if it is not,
that is signal for M5's live push, not a reason to build it now).
