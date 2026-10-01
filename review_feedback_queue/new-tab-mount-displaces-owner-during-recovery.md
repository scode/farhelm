# A recovering view silently takes the session back when a new tab appears

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Say you had a session open on one device (a laptop that went to sleep, or dropped off the network), then took it over
from a second device and opened a tab there. When the first device comes back, its page attaches the new tab on its own.
That quietly takes the whole session away from the device you are actively using: its terminals flip to "taken over". No
one on the first device asked for this, and SPEC.md forbids it.

## Details

Source: gap-filling review pass, 2026-09-30, slice ui-terminal.

Reviewer's confidence: confirmed (traced end to end in the UI, the helm and the supervisor; the only precondition is
that another client opens a tab while this view is recovering).

Reviewer's bucket suggestion: high.

Possible cover for triage to check: none (checked SPEC.md, the TODO.md Planned bucket, the queue and TRIAGE_OUTCOMES.md;
the multi-helm TODO entry is about separate helms, not this path)

- `crates/farhelm-ui/assets/terminal.js` `sync()` (about lines 2564-2610) mounts every spec it has not seen before with
  `farhelmTerm.mountWhenReady(spec, baseUrl, attach)`. There is no `ifUnowned` argument, so `mount()` (lines 3331-3336)
  connects on `spec.path`, the displacing route, not `spec.pathUnowned`.
- Only two things stop that mount:
  - the takeover latch (`takeover`), which is set only after a taken-over notice actually arrives on one of this view's
    sockets;
  - `reconnecting(spec.el)`, which covers only elements that already have a recovery controller.
- A view whose sockets were dead when the other client took over got no notice, so it is not latched. It is exactly the
  state the header's "An unattended attach must not steal the session" section describes. Its existing terminals recover
  correctly through `if_unowned`.
- A tab that first appears in the session detail during that window has no controller and no latch. The reconciler
  mounts it with an ordinary displacing attach under the view's lease.
- The supervisor then sweeps every attachment of any other lease (`crates/farhelm-supervisor/src/service/handlers.rs`
  about lines 1892-1905; `displaced_by_attach` in `service/terminals.rs:580`). The owner is evicted with a taken-over
  notice.
- It gets worse from there. The recovering view's own next `if_unowned` probes now find their own lease in charge and
  are accepted, so the view reclaims the agent terminal too.

This is close to deterministic after a longer outage:

- Once the 30 s retry ladder is spent, terminals probe only every 30 s (`reconnect.rs` `PROBE_INTERVAL_MS`).
- While the event feed is down, the page falls back to a poll every `POLL_INTERVAL_MS` (a few seconds; `feed.rs:254`).
- When connectivity returns, the poll almost always sees the new tab before the next terminal probe gets its refusal and
  latches the view.

SPEC.md lines 912-916 ("A terminal recovering on its own never TAKES the session… Taking a session over stays a thing
someone does on purpose") and the header's own "silently steals it back" paragraph describe exactly this outcome.

Other ways the same path fires: a tab beyond the 32-island cap becoming mountable after a sibling closes. A tombstone
buried by an identity change also mounts displacing.

To verify: open a session on client A and hold A's sockets down (e.g. `__farhelmTestReconnect.delaysMs` with a long
ladder plus a network block). Attach from client B, open a tab there, restore A's connection, and watch B receive a
taken-over detach.

Fix: when the view does not currently know it holds the session, mount newly discovered specs on the unattended route.
For example, pass `ifUnowned` when any recovery controller exists or when a spec was not user-initiated. Treat a refusal
as the latch, as the ladder already does.

Related: `takeover-latch-misses-attaching-tabs.md` (from a separate review) has the same root, the reconciler mounting
newly seen tabs on the displacing route, with a different trigger: the takeover latch fires while an attach is already
in flight. Each item's fix misses the other's case (that item's fix acts only when a latch fires, which never happens
here; this item's fix covers poll-discovered tabs but not a tab the user opened), so they are best fixed together. Also
check the interaction with `terminal-tombstone-never-buried.md`: its fix adds burial when a terminal departs, and a
buried tombstone is remounted on the displacing route.
