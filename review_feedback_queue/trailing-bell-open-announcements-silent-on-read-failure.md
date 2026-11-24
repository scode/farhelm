# Open Announcements in the bell does nothing visible when its read fails

Reviewed commit: 4e13aff403c4823cfa7a2f35b645c2df176641e2

## TLDR

In the notification bell, pressing Open Announcements on a New announcement notice can do nothing at all: if the helm
cannot be read (connection down, refusal, unreadable answer) the popover stays open, no dialog appears and no message
says why. Briefly after another client dismissed the batch, the same silent no-op happens with an empty list.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Narrowed at recording: the empty-list case (a brief window after another client dismissed the batch)
matches FILTER.md "Rare, self-correcting glitches and imprecise diagnostics", which the independent drop check agreed
with; the error path does not, since a failed read is not shown to be rare in the filter's sense. The checker also noted
that `read_pending` returns with no log at all if `ApiPath::api` fails (announcements.rs:141-143), probably unreachable
with fixed path segments.

### Reviewer record (`g09-announcements-gui/f1`)

Location: `crates/gui/farhelm-gui/src/announcements.rs:145`. Summary: Bell's Open Announcements does nothing visible
when its pending read fails or comes back empty

Revision (turn 2): checked the helm side (g08 interface) to bound the empty case. The helm resolves the notice in the
same transaction that empties the queue (crates/farhelm-helm/src/announcements.rs:137-161 calling
crates/farhelm-helm/src/notices.rs:172-180,
`resolved = 1 ... AND NOT EXISTS (SELECT 1 FROM announcements WHERE shown_at IS NULL)`), and a notice is raised only
when a network enqueue inserted rows (announcements.rs:101-103). So the empty-but-unresolved state is only the short
window before the resolving push reaches this window (another client dismissed the batch). The failure case (helm
unreachable, refusal, unreadable answer, read never sent) is the main path and is unchanged. Bucket and fix unchanged.

Reviewed commit: 4e13aff403c4823cfa7a2f35b645c2df176641e2 (tip e159ffc79ab147be44e92a92b59571650a4f322d still has it).

TLDR: In the notification bell, pressing Open Announcements on a New announcement notice can do nothing at all: if the
helm cannot be read (connection down, refusal, unreadable answer) the popover stays open, no dialog appears and no
message says why. Briefly after another client dismissed the batch, the same silent no-op happens with an empty list.

Details: `NoticesButton::open_announcements` (crates/gui/farhelm-gui/src/rail_notices.rs:240-264) calls
`announcements::read_pending` (crates/gui/farhelm-gui/src/announcements.rs:140-155). On `Err(words)` that function only
logs `tracing::warn!("could not load pending announcements")`; if `ApiPath::api` fails it returns silently; and it
ignores the bool `operations::read` returns (crates/gui/farhelm-gui/src/operations.rs:785-796), so a read that was never
sent also ends silently. On success with an empty list, rail_notices.rs:248 returns without closing the panel or saying
anything. SPEC.md, "Errors and diagnostics": "Every failed operation surfaces a concrete, actionable error in the
client." Its best-effort exceptions name "the automatic announcement check and recording a dismissed batch", not the
user-pressed bell action. SPEC.md (rail notices): "**Open Announcements** loads the current pending batch and closes the
popover." Fix: keep a refusal line in the panel (as Clear resolved keeps `refusal`) and show the helm's words on failure
or when `read` returns false; on an empty batch close the panel or say the announcements were already read; consider an
in-flight state so repeated presses do not stack GETs.

Suggested bucket: other — a confusing dead button on a rare failure path, no data or security impact.

Confidence: confirmed (source shows no UI on the error/empty paths).

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g09-announcements-gui/f1`.
