# Desktop smoke cleanup sends KILL after its group disappears

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Desktop smoke cleanup sends KILL after its group disappears.

## Details

`F40 / COR-SMOKE-STALE-PGID` — **possible** — `scripts/desktop-smoke.sh:237` — Desktop smoke cleanup sends KILL after
its group disappears

The desktop smoke check starts an answering supervisor in its own process group so cleanup can stop both the supervisor
and its startup children. Cleanup sends TERM to that group, polls until the group disappears or ten seconds pass, and
then sends KILL unconditionally. Even the successful “group is gone” path reaches KILL using the old group number.

If the original group disappears and its number is reused before that final signal, cleanup could kill an unrelated
process group. Waiting for the supervisor afterward does not establish ownership at the time of KILL: Bash can reap
background children before the explicit `wait`. The required reuse is extremely narrow and was not observed. The
relevant window is between disappearance and KILL, not necessarily the full ten-second grace period. Stop escalating
once the group disappears, and preserve process ownership for any remaining cleanup that needs it. Proposed bucket:
highest. No possible cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_general p1`, `auto_data p1`, `auto_lifecycle p1`, `auto_security p1`,
`auto_edges p1`, `auto_secrets p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Extremely narrow unobserved PGIDreuse premise; grace10sec differs absence-to-KILL interval.

## Filed reviewer metadata

- `auto_general p1`: confidence as filed: possible / likely only if a new process group reuses the old group ID before
  the unconditional late KILL. Severity: process loss. Suggested bucket: highest. Suggested bucket as filed: highest.
- `auto_data p1`: confidence as filed: possible; open premise is sufficiently rapid PGID reuse after the original group
  disappears. Suggested bucket as filed: highest (possible loss of unrelated processes). Confidence: possible; open
  premise is sufficiently rapid PGID reuse after the original group disappears.
- `auto_lifecycle p1`: confidence as filed: possible; confirmed unconditional negative-PGID KILL after the absence test,
  unverified premise is disappearance/reaping and reuse of that group id before KILL. No observed reproduction.
  Suggested bucket as filed: highest.
- `auto_edges p1`: confidence as filed: possible; recycled PGID before the final signal is unverified. Severity: loss of
  unrelated process groups/work. Suggested bucket: highest. Suggested bucket as filed: highest.
- `auto_security p1`: confidence as filed: possible / likely with the unverified premise that the supervisor group
  disappears and its group id is reused before an escalation signal. No reproduction. Suggested bucket as filed:
  highest. Confidence: possible / likely with the unverified premise that the supervisor group disappears and its group
  id is reused before an escalation signal. No reproduction.
- `auto_secrets p1`: confidence as filed: possible; unverified premise that the original group becomes empty, Bash reaps
  its exited leader, and the group number is reused before a later probe or signal. Suggested bucket as filed: highest.
