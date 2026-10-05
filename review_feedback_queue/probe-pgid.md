# Probe cleanup signals group after releasing its numeric identity

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Probe cleanup signals group after releasing its numeric identity.

## Details

`F4 / COR-PROBE-PGID` — **possible** — `crates/farhelm-helm/src/provisioning/backend.rs:1978` — Probe cleanup signals
group after releasing its numeric identity

When a host probe fails before completing its handshake, the helm waits for and reaps the direct probe process. It then
sends `SIGKILL` to the process group identified by the child’s saved numeric process ID, to remove helpers such as an
SSH `ProxyCommand`.

That saved number is safe while an original group member remains alive. The cleanup runs unconditionally, however,
including when the original group is already empty. After the direct child has been reaped and the group has
disappeared, the operating system can reuse the number. If an unrelated process group belonging to the same account
acquires it before the signal, a failed host probe could kill unrelated local work. The code comment explains why a
surviving helper protects the number, but that explanation does not cover the empty-group case.

Keep the direct child’s identity reserved until group cleanup finishes. One approach is to observe termination without
reaping, using `WNOWAIT`, then signal the group and finally reap the child. Do not signal a cached group number after
releasing the identity that made it safe.

Suggested bucket: **highest**. No possible cover was identified. The required process-ID reuse and scheduling
interleaving have not been demonstrated. The earlier helper-cleanup entry at `TRIAGE_OUTCOMES.md:5910–5929` concerns a
different trigger.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `helm_data p1`, `helm_secrets p2`.

Possible cover recorded during collection: none identified.

Collection caveats: PIDreuse scheduling premise unverified; helpercleanup ledger5910–5929 different trigger.

## Filed reviewer metadata

- `helm_secrets p2`: confidence as filed: **possible; likely under the rare premise that the now-empty process-group
  number is reused between reaping and signaling**. The unsafe ordering is confirmed by source; no runtime reproduction.
  Suggested bucket as filed: loss of unrelated processes or user work / **highest**.
- `helm_data p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
