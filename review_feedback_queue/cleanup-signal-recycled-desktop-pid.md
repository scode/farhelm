# Cleanup can signal a recycled desktop PID

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Desktop smoke cleanup could signal a recycled process number.

## Details

F70 — **possible** — `scripts/desktop-smoke.sh:266`; `scripts/desktop-smoke.sh:265–267`;
`scripts/desktop-smoke.sh:1120–1125` — Cleanup can signal a recycled desktop PID

Normal completion reaps the desktop process but leaves its identifier in the cleanup record. Later asynchronous work and
another scenario can run before final cleanup signals that saved number. If it has been reused, an unrelated process can
receive the signal; no such loss was reproduced. Retire the record when reaping the desktop and preserve
process-instance ownership for children that exit asynchronously.

## Evidence and triage context

- scripts/desktop-smoke.sh:1120 reaps the desktop; :1125 passes the number from desktop.pid without clearing it. Lines
  1145–1202 launch and await another supervisor and desktop. Teardown at :265–267 subsequently signals the retained
  number. The release workflow invokes this script at .github/workflows/release.yml:301.
- scripts/desktop-smoke.sh:1120–1125 waits for the original desktop without removing desktop.pid; :1145–1202 performs
  another scenario; :265–267 reads and signals the old PID during exit cleanup.
- scripts/desktop-smoke.sh:1120 waits for the process passed at :1125; desktop.pid remains populated. Lines 1145–1202
  then perform another scenario. Exit teardown at :266 signals the number read from that unchanged file.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6598–6608 covers the immediate process-group check/KILL gap, not this saved desktop PID.
  SPEC_impl.md:2201–2202 explicitly excludes numbers retained across asynchronous work.
- SPEC_impl.md:2194–2205 accepts short reuse windows but expressly excludes this retained-number lifetime.
  TRIAGE_OUTCOMES.md:6598–6608 concerns the distinct immediate group-signal site.
- SPEC_impl.md:2201–2202 excludes stored numbers carried through asynchronous work. TRIAGE_OUTCOMES.md:6598–6608 accepts
  a different, immediate group-number race.

Caveats:

- PID reuse and unrelated-process termination were not reproduced. Test tooling only.
- Unrelated PID reuse remains unverified; the stale record itself is established.
- Requires PID reuse before teardown. No process loss was reproduced.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_08_cor:p1:F1`,
`automation_website_08_sec:p1:F1`, `automation_website_09_sec:p1:F2`.

- `automation_website_08_cor:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
- `automation_website_08_sec:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
- `automation_website_09_sec:p1:F2`: confidence as filed: possible; suggested bucket as filed: highest.
