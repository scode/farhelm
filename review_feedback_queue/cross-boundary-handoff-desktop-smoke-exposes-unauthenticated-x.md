# Cross-boundary handoff: desktop smoke exposes an unauthenticated X display

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The desktop smoke display could allow another account to observe or inject input.

## Details

F71 — **possible** — `scripts/desktop-smoke.sh:583–584`; `scripts/desktop-smoke.sh:583` — Cross-boundary handoff:
desktop smoke exposes an unauthenticated X display

The smoke test starts a local X display without authentication. Private product-state files and disabled TCP listening
do not authenticate clients connecting through its local socket. Another account able to access that socket could
observe the desktop or inject terminal input with the runner's authority. Shared-machine access and exploitation were
not tested. Use a private per-run Xauthority cookie for the display server and every client.

## Evidence and triage context

- scripts/desktop-smoke.sh:583–584 launches Xvfb with -nolisten tcp but no -auth. Lines 589 and 593–602 attach the
  window manager and desktop. Lines 761–769 create real Bash sessions. Lines 68–73 explicitly acknowledge cross-account
  Unix-socket access.
- scripts/desktop-smoke.sh:583–584 starts Xvfb without -auth; :593–602 launches the desktop on that display; :761–769
  creates Bash sessions. The exposure is acknowledged at :68–73. The native smoke is a real release caller at
  .github/workflows/release.yml:296–306.
- scripts/desktop-smoke.sh:583–584 supplies no X authentication; :589–602 connects the window manager and application;
  :761–769 starts real Bash sessions. Lines 68–73 expressly describe access by other local accounts.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2208–2213 excludes other local accounts for the browser UI on unsuitable machines, while expressly
  distinguishing the native app. That does not clearly accept an unauthenticated native-test display.
- The single-user-box comment at scripts/desktop-smoke.sh:71–73 is not a specification or recorded acceptance.
  SPEC.md:2208–2213's browser-specific exclusion does not clearly cover this harness.
- SPEC.md:2152–2164 trusts deliberate same-account interference only. SPEC.md:2208–2213's browser exclusion is not exact
  coverage of this native-test display.

Caveats:

- Requires another local account able to access the X socket during a run. No cross-account connection or injection was
  executed.
- Shared-machine deployment and successful cross-account access remain untested.
- No cross-account exploit was reproduced, and a hostile local account must exist on the machine.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_08_sec:p1:F3`,
`automation_website_09_cor:p1:F4`, `automation_website_09_sec:p1:F1`.

- `automation_website_08_sec:p1:F3`: confidence as filed: possible; suggested bucket as filed: highest.
- `automation_website_09_cor:p1:F4`: confidence as filed: possible; suggested bucket as filed: highest.
- `automation_website_09_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
