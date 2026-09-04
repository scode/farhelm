# Session dot read state: grey idle, blue unseen, mark read/unread

Anchor: commit `1e4a1340fa29ddc65b3373feb80e5a2432e78e82`, 2026-09-03.

This plan covers three TODO.md entries under "near term" at once: the grey idle dot, the blue "idle with unseen output"
dot, and the mark read / mark unread toggle. They are one feature with three visible pieces. The grey entry is worded
"idle and its last output has been seen", and nothing in the system today knows whether anything has been seen, so the
first entry cannot ship without the state the other two need. Effort: medium. That word rests on the helm side being a
small table, one route, and one row field in a shape the code already has three examples of (preferences, archive,
profiles), and on the UI side being a badge variant, an effect, and one menu item, with the spec edits and the browser
tests being most of the remaining work.

NOTE: this is a plan, not a record of what was built. It is written against the anchor commit and is allowed to go
stale; check `git log <anchor>..main` over the paths below before executing it.

## Decisions already taken

Settled with the maintainer on 2026-09-03, before this plan was written:

- One plan and one PR stack for all three entries, not three plans.
- The seen state lives in the helm's database, per session, shared by every client — the same model SPEC.md's Session
  list section already prescribes for the sort order and the last-selected session ("no client keeps its own copy"). Not
  on the supervisor's session row (it is a viewer fact, not a session fact, and would cost a protocol change), and not
  per client (SPEC.md forbids that outright).
- "Seen" means the session is the open one in some client: opening it counts, and output arriving while it stays open
  counts. Window focus and tab visibility are ignored. A session open in a hidden tab is considered seen; that is a
  known imprecision accepted for the sake of not having two engines answer visibility differently.
- The unseen-idle dot is blue. Waiting, which draws blue today (`--info`), moves to red: it is the one live status that
  is a request for a human, so it belongs with the attention colours, not beside "nothing is wrong". The maintainer
  noted never having seen a waiting dot in practice; see the closing note on that.

## The model

The helm keeps, per session id, the activity stamp that was current the last time the session was seen. Call it
`seen_activity_at`. It is compared against the session's effective activity stamp (`SessionInfo::effective_activity`,
the same value the list already sorts by and the row already renders as an age), and a session has unseen output exactly
when its effective activity is newer than its `seen_activity_at`. A session with no recorded stamp has never been seen.

Storing the activity stamp that was seen, rather than a wall-clock "seen at" time, is the load-bearing choice. The
activity stamp is written by the session's host and the helm would otherwise be comparing it against its own clock,
which on a remote host is a different machine's clock; SPEC_impl.md already spends a paragraph refusing to do that for
the relative age. Comparing the host's stamp against an earlier copy of the same host's stamp involves no clock at all
and cannot go wrong across machines. Two consequences to accept and document:

- The supervisor quantizes `last_activity_at` at the source, moving it only when what it saw is at least a minute newer
  than what it holds (the field's proto docs). Output that lands within that minute after a mark-read does not register
  as unseen. This is cosmetic and matches the field's own "activity around this time" contract; it is not worth a
  second, finer stamp.
- A session whose helm predates `last_activity_at` (effective activity falls back to `created_at`) can only ever be
  unseen once, at creation. Same as above: acceptable, and such helms are no longer in the field.

The three dot states then derive from status plus that predicate, on the client:

| status  | unseen output | dot                         |
| ------- | ------------- | --------------------------- |
| running | either        | green, pulsing (unchanged)  |
| waiting | either        | red, still (was blue)       |
| idle    | no            | grey, still (was dim green) |
| idle    | yes           | blue, still (new)           |

Running and waiting do not show the unseen state. The entry asks for it on idle only, and a pulsing green dot already
says "look here" on its own. A stale row (host unreachable) keeps whatever verdict the last report gives, still, as
today.

### Marking, automatic and manual

The client marks a session seen (sets `seen_activity_at` to the session's current effective activity) in exactly two
situations: when the session becomes the open one, and when the open session's effective activity advances. Both are one
effect in the session view keyed on the pair (session id, effective activity), so the effect fires on mount and on every
stamp change and on nothing else. Keying it on the stamp rather than on "is there unseen output" is what makes a manual
mark-unread on the OPEN session stick: the stamp has not moved, so the effect does not re-fire, and the session stays
unread until the user navigates away and back or the agent produces new output. That rule should be stated in SPEC.md,
since a user will discover it.

The effect must not fire for a stale session (the session view shows the host-unreachable band; nothing is being looked
at) nor for an ended one (there is no unseen state to clear, and writing a stamp for an exited session is harmless but
pointless; skip it to keep the write set small).

Manual marking is the toggle: "mark unread" clears the stamp (the row then reads unseen, because any activity is newer
than nothing), "mark read" sets it to the current effective activity. The toggle is reachable from the row's `…` menu
and by clicking the dot itself. Which word the menu shows follows the current predicate: "mark read" when the row has
unseen output, "mark unread" otherwise.

## Helm side

### `crates/farhelm-helm/src/store.rs`

Schema version 16: a new table, not a column on `session_cache`.

```sql
CREATE TABLE session_seen (
    session_id       TEXT NOT NULL PRIMARY KEY,
    seen_activity_at INTEGER NOT NULL
) STRICT;
```

`session_cache` rows are replaced wholesale by `replace_host_sessions` on every host refresh under the changed-only
rule, and a column there would need that write to preserve one field it did not receive from the host, which puts a
viewer fact inside the supervisor-payload mirror and complicates the one write the cache has. A separate table keyed by
session id alone (session ids are supervisor-minted UUIDs; the cache already enforces one host per id) needs no join
with the host and survives a retarget or an adoption, both of which are supposed to keep the session.

Rows are deleted when the session is deleted through this helm (`sessions::delete_session`). A session deleted by
another helm, or one that leaves a cache because its host was removed, leaves a row behind. That garbage is bounded by
the number of sessions that ever existed, at a few dozen bytes each, and a later migration can sweep rows whose id no
`session_cache` row names; not worth a reaper now. Say so in the table comment.

Store API, in the style of `preferences`/`update_preferences`: `seen_activity(&self, ids)` returning a map for the
listing join, `mark_seen(&self, session_id, activity_at)` (upsert), `clear_seen(&self, session_id)` (delete), and the
delete-session path removing the row. Add the table to the schema-history doc comment (the list ending at version 15
around store.rs:1100), to the fresh-create branch, to the migration ladder, and to the downgrade-test drop lists near
store.rs:5140 to 5280, which enumerate every table a rollback test tears down. Unit tests: the upsert and clear
round-trip, a listing join for ids with and without rows, and the migration from a version-15 database.

### `crates/farhelm-helm/src/aggregate.rs`

`SessionRow` gains `seen_activity_at: Option<i64>`, ALWAYS serialized (`null` for never seen), the same shape and the
same reason as `host_identity`: a client must be able to tell "this helm says never seen" from "this helm predates the
field", because only the former should draw blue and offer the toggle. `session_list_staged` reads the map once per
listing and joins by id while it builds rows; the read goes through the store beside the cache reads, never to a host.
Serialization test for the key being present and null.

### `crates/farhelm-helm/src/sessions.rs` and `lib.rs`

One route, `PUT /api/sessions/{id}/seen`, body `{"seen_activity_at": <i64>}` to mark seen, `{"seen_activity_at": null}`
to mark unread. It answers 404 through the same "no host has ever reported this id" lookup the other per-session routes
use (the row must exist in some cache or the manager's list; the seen table is not consulted for existence) but does NOT
go through `route_session`: marking a session on an unreachable host is a helm-local write with nothing to refuse. After
the write, `state.manager.events().bump()` so every other client re-reads its list and redraws the dot; profiles.rs is
the model, at profiles.rs:188. Delete-session removes the row before bumping.

Tests in `sessions_tests.rs` through `rest_harness`: mark then list shows the stamp; clear then list shows null; unknown
id is 404; each write bumps the revision; delete drops the row. The events tests already pin that a no-op refresh does
not bump, so a PUT that writes the value already stored may either bump or not; choose not, and test it, since a client
that re-marks on every stamp change would otherwise ping every other client on every one of its own redraws.

### Downgrade

A version-16 database is refused by a version-15 binary, like every schema bump before it. The pre-upgrade backup entry
under "maybe later" is the general answer; this plan adds nothing for it beyond mentioning the bump in the release
notes.

## UI side

### `crates/farhelm-ui/src/lib.rs`

`Session` gains `seen_activity_at: Option<Option<i64>>` decoded through the existing `double_option` helper at
lib.rs:547 (it is `String`-typed today; generalize it or add an `i64` twin). A predicate `Session::has_unseen_output()`
returns `Some(bool)` when the helm sent the key (`None` for an old helm, which draws the pre-plan colours and offers no
toggle), true when the inner value is `None` or older than `effective_activity()`. Unit tests beside
`effective_activity_falls_back_to_creation_time_only_when_unknown`.

### `crates/farhelm-ui/src/status.rs`

`status_badge` takes the unseen verdict alongside the status and annotation. For `Idle` with unseen output it yields
class `idle unseen` and text `idle — new output`; every other status is unchanged. The text change matters as much as
the class: the hidden word is what screen readers and the browser suite read, and "new output" is a fact the colour
alone would otherwise carry. `StatusBadgeView` is unchanged; the row, not the badge, owns the click (below). Extend the
per-status table test and the `an_unknown_status_produces_no_badge_at_all` family with the unseen cases, including that
running and waiting ignore the flag.

### `crates/farhelm-ui/assets/app.css`

Colour changes, with the rationale comments rewritten rather than left describing the old grouping (the "two groups, not
three" paragraph at app.css:1939 is the one that becomes false):

- `.status-badge.idle` from `var(--ok-dim)` to a muted foreground token (the one `exited` uses, `--fg-1`, or one notch
  dimmer; pick against the row surfaces so it is visibly not green and not "disabled"). `--ok-dim` then has no user;
  remove it or leave it with an updated comment.
- `.status-badge.idle.unseen` is `var(--info)`; the token's comment becomes "idle with output nobody has looked at".
- `.status-badge.waiting` goes red. `--danger` and `--danger-strong` are nearly the same red, and `error` already draws
  `--danger-strong` as a WORD; a waiting DOT in the same family is distinguishable by shape, and the comment should say
  that is the argument. Do not reuse amber: interrupted owns it.
- A clickable dot needs a cursor and a focus ring; the dot in the row becomes a button (below) and the existing
  `.status-dot` sizing must survive being inside one.

The session header and the stale metadata band render the same badge and pick up the colours for free.

### `crates/farhelm-ui/src/list/row.rs`

Two additions. First, `MenuAction::MarkSeen` (one action, two labels), inserted after `Rename` in `MENU_ACTIONS` and
offered whenever the helm sent the field and the row's status is live (running, waiting, idle); ended rows have no dot
and no meaningful unseen state, and `session_menu_order` is where that predicate goes. The label is "mark read" when the
row has unseen output, "mark unread" otherwise. The click hands the row's id and the target value (current effective
activity, or none) up to the view like `on_archive` does.

Second, the dot click. The row's badge is rendered by `StatusBadgeView` with an `aria-hidden` dot; the row wraps that
dot in a `button` with an `aria-label` of "mark read"/"mark unread", stops propagation so the click does not select the
row (selection would mark it read on its own and defeat "mark unread"), and calls the same handler. Only the row's dot
is clickable; the header and stale band render the plain badge. Rows without the field, and ended rows, render the plain
badge too.

### `crates/farhelm-ui/src/list/view.rs` and `api.rs`

An `api::mark_seen(base, id, Option<i64>)` doing the PUT, fire-and-forget with the same failure logging the other
per-session posts use; no optimistic local state is needed because the events bump re-reads the list within one
round-trip, but if the redraw visibly lags the click, apply the same optimistic-then-settle shape the renames use
(`rows::apply_optimistic_renames`). The view wires the menu and dot handlers to it.

### `crates/farhelm-ui/src/session_view.rs`

The auto-mark effect, near the existing `use_effect` at session_view.rs:812: keyed on (session id, effective activity),
skipped for stale and ended sessions and when the helm sent no field, calling `api::mark_seen` with the stamp. Document
in the effect's comment why it is keyed on the stamp and not on the predicate (the mark-unread-while-open rule above).

## Spec and docs

`SPEC.md`, Status section: the dot paragraph gains the four-state table in prose (running pulses green; waiting red;
idle grey; idle with output not yet seen blue), the definition of seen, the two automatic marks and the manual toggle,
and the rule that the state is helm-kept and shared like the list preferences. Session list section: "a row shows … its
status" stays; add the read/unread toggle to what the row menu offers. `SPEC_impl.md`, Helm internals: the table, why it
is not a `session_cache` column, why the stored value is an activity stamp and not a clock reading, the minute
quantization caveat, and the events bump. `docs/` has no user-facing page for the sidebar today; none is added.

## Browser tests

`e2e/tests/terminal.spec.ts` has the two dot tests (`a live session draws a dot with a hidden word and a relative age`
at line 3493, `only a reachable running dot pulses` at 3594); the first pins the hidden word and will need the idle
cases. New cases in `e2e/tests/sidebar.spec.ts`: an idle session that produced output while another session was open
draws blue and reads "idle — new output"; opening it turns it grey within one feed round-trip; mark unread from the menu
turns it blue while it stays open; the dot click toggles without changing the selection; a second page (the two-client
shape the preferences tests use) sees the same verdict. Waiting is hard to provoke end-to-end (it comes only from
per-kind sharpening); assert its colour through the class only, as today.

## PR shape

Four PRs, bottom up: helm store and route and row field (with the spec-impl paragraph); UI model, badge, colours, and
the auto-mark effect (this is the PR that makes idle grey and delivers the first two entries); the menu item and dot
click (the third entry); SPEC.md wording, which can ride with the second PR if the stack is being kept short. Remove the
three TODO entries and this file in the last PR.

## Notes for whoever executes this

The maintainer has never seen a waiting dot. Waiting comes only from per-kind sharpening in
`crates/farhelm-supervisor/src/service/status.rs` (around line 328), so if that sharpening never fires for the agents in
use, the red is moot in practice and the colour change is still correct. Whether the sharpening works is a separate
question, not part of this plan.

The helm's preference writes (`api::store_preference`) go through a serialized per-field queue with replay after
credential recovery. Seen marks do not need that machinery: a lost mark is corrected by the next open or the next
output, and a lost mark-unread is a one-click redo. Do not extend the queue for this.
