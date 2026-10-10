# Lowercase sections can override ownership despite being ignored by systemd

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A lowercase section typo could mislead uninstall about service ownership.

## Details

F13 — **possible** — `crates/farhelm-helm/src/units.rs:238` — Lowercase sections can override ownership despite being
ignored by systemd

Farhelm treats `[service]` like `[Service]`, while systemd ignores the lowercase section. A later lowercase section can
therefore replace Farhelm's ownership path even though the real service still executes the earlier path. In an edited,
marked unit, uninstall could stop and remove another installation's service. Shipped templates use the correct spelling;
acceptance of accidental edits remains the uncertain premise. Match the section name case-sensitively and correct the
test that requires case-insensitive parsing.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:238 uses eq_ignore_ascii_case("Service"); :252 replaces the previously parsed
  executable.
- For [Service] naming A followed by [service] naming B, Farhelm returns B.
- https://github.com/systemd/systemd/blob/v255/src/shared/conf-parser.c#L209 preserves the section spelling; :216 checks
  exact membership in the known-section list; :229–233 ignores unknown sections.
- crates/farhelm/src/setup.rs:501–517 can consequently classify A's marked service as B's; :898 and :914 stop and remove
  it.
- crates/farhelm-helm/src/units.rs:1057–1060 explicitly assert the incorrect lowercase-section behavior.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "SPEC.md:2160–2164", "comparison": "A deliberately planted misleading section would be excluded. A typo
  introduced while editing a genuine setup-owned unit is not clearly the same trigger."}

Caveats:

- Requires an edited setup-marked base unit containing a valid Service section and a misleading case-variant section.
- Shipped templates use the correct section name.
- No destructive reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_12_cor:p1:F4`.

- `helm_state_provisioning_12_cor:p1:F4`: confidence as filed: possible; suggested bucket as filed: highest.
