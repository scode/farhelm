# notify_blocked falls back to the raw asking session id instead of refusing an id the helm cannot represent

Reviewed commit: 721dd96fe7315659b661faa0ede8656d5f4192a1

## TLDR

A broken or hostile host can report a blocked agent for a session id that Farhelm otherwise refuses to accept (empty,
over 40 bytes, with control characters, or a dot segment). Instead of refusing it, the helm records the notice anyway.
The notice then carries a session id in a form no other part of Farhelm produces, and the bell shows the raw id as the
session's name. Every other path that acts on the asking session refuses such an id.

## Details

In notify_blocked (crates/farhelm-helm/src/agent_requests.rs:1334-1336 at the tip),
`SessionRef::new(entry_uuid, session_id)` fails exactly when `bare_session_id_problem` reports the id
(crates/farhelm-proto/src/session_ref.rs:37-50, 100-110), and the code then uses
`unwrap_or_else(|_| session_id.to_owned())`. The id is still used as the notice key (condition_key), as the title
fallback when the session is not cached (and an inadmissible id can never be cached, because session_cache.rs:49 refuses
such rows), and as `NoticeContent::AgentBlocked::session_id`. That field is documented as "The session's id as clients
know it: its host entry and bare id, as `GET /api/sessions` serves it" (crates/farhelm-proto/src/notices.rs:35-38).
Every other asking-session path refuses instead: resolve_target refuses with "this session ID cannot be represented by
the helm" (agent_requests.rs:1581-1586), the approval path propagates the SessionRef error with `?` (approvals.rs:451),
and the sessions listing skips such rows (agent_requests.rs:5088). The id comes from the host's supervisor, which the
helm does not trust, and the relay bounds it only at MAX_SESSION_ID_BYTES (1024). Display is not unsafe, because the
bell escapes host text (gui host_text.rs:89-92) and the ntfy title goes through clean_text. The harm is the broken field
contract (a client that parses session_id as a SessionRef gets an error, or a bare id that may collide across hosts),
plus a notice and an alert for a session id the helm treats as nonexistent. Verify by reading the three sites above side
by side. The fix is to refuse the verb, as resolve_target does, when `bare_session_id_problem(session_id)` is Some,
before recording anything. Optionally pin it with a test that a Notify from an inadmissible asking id is refused and
records no notice.

### Triage notes

- Recorded bucket: `other`. Possible partial cover: plans/queue/blocked-notify-attention-and-debounce.md (not landed)
  removes the bell's blocked notices, which would remove the stored bare id; it does not say the asking session id gets
  validated.
- Suggested bucket (reviewer): other. Only a broken or hostile supervisor can trigger it, storage is bounded per host,
  and display is escaped. It is a contract inconsistency, not a loss of data or authority.
- Confidence: confirmed (from source) that the fallback stores a non-qualified id where every other path refuses. That
  no current client parses AgentBlocked.session_id is also from source (rail_notices.rs:743-747 ignores it).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g20/f2 (finding), group g20 "An agent telling the user it is blocked: helm, protocol, CLI", findings log line 9.
