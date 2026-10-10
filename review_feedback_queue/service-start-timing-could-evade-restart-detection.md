# Service-start timing could evade restart detection

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Setup could miss a service that starts from its old definition.

## Details

F119 — **possible** — `crates/farhelm/src/setup.rs:743` — Service-start timing could evade restart detection

Setup's restart decision relies on an earlier inactive-status observation. A service starting from the old definition
afterward could miss the restart marker and remain on old arguments while setup reports success. Whether normal systemd
transitions realize this ordering across reload and enable remains unverified. Establish that lifecycle behavior, then
make the restart decision account for intervening starts or conservatively restart affected services when required.

## Evidence and triage context

- setup.rs:743–747 records a restart only when is-active currently returns active. :1361–1364 interprets status 3 as
  inactive. Publication at :750 precedes reload at :758 and enable --now at :763–765; :778–802 restarts only with an
  existing marker.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- SPEC.md:417–427 requires correct lifecycle outcomes, but does not explicitly accept stale running configuration.
  TRIAGE_OUTCOMES.md:3747–3762 covers paired write failures, not this observation/start race. FILTER.md:24–42 does not
  clearly cover success with persistently stale runtime state.

Caveats:

- No runtime or external systemd lifecycle verification. Shipped Type=simple narrows startup timing but does not itself
  prove that no start can occur after the earlier status observation. No user-process loss established.
- The systemd transition across daemon-reload and enable --now is the unverified premise. No runtime reproduction or
  process loss established; Type=simple alone does not refute the race.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_02_cor:p1:C7`.

- `cli_installation_02_cor:p1:C7`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.
