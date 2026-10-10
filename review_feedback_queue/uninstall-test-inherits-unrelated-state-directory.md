# Uninstall test inherits an unrelated state directory

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The uninstall test fails under a valid inherited state-directory setting.

## Details

F121 — **definite** — `crates/farhelm-helm/src/provisioning.rs:5792` — Uninstall test inherits an unrelated state
directory

The fixture expects state under its temporary home but lets its child inherit an absolute state-directory setting
pointing elsewhere. Correct uninstall inspection then contradicts the hard-coded expectation and produces a false test
failure. Remove that setting from the fixture child's environment, or provide a fixture-owned value and assert it. Do
not change the environment of the process running the tests.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning.rs:268–278 constructs a child shell, overrides HOME and PATH, and removes
  XDG_CONFIG_HOME, but preserves XDG_STATE_HOME.
- crates/farhelm-helm/src/provisioning.rs:5735–5744 selects that launcher; lines 5770–5777 invoke the real uninstall
  inspection.
- crates/farhelm-helm/src/provisioning/backend.rs:2444–2446 returns an absolute inherited XDG_STATE_HOME plus /farhelm.
- crates/farhelm-helm/src/provisioning.rs:5792–5795 instead requires the temporary HOME's .local/state/farhelm.
- scripts/record-test-run.py:1212–1220 removes unrequested FARHELM_ variables only, preserving XDG_STATE_HOME.
- SPEC_impl.md:3381 explicitly requires resolution using the host environment, including XDG_STATE_HOME.
- crates/farhelm-helm/src/provisioning.rs:268–278 inherits XDG_STATE_HOME while overriding HOME.
- crates/farhelm-helm/src/provisioning.rs:5738–5741 installs that launcher in the backend used by the test.
- crates/farhelm-helm/src/provisioning/backend.rs:2444–2446 honors absolute XDG_STATE_HOME.
- crates/farhelm-helm/src/provisioning.rs:5792–5795 unconditionally expects home.join(".local/state/farhelm").
- scripts/record-test-run.py:1212–1220 preserves XDG_STATE_HOME; SPEC_impl.md:3381 confirms that honoring it is correct
  product behavior.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires an absolute XDG_STATE_HOME resolving to a different expected path; unset or relative values take the HOME
  fallback.
- No runtime reproduction.
- No product-state mutation or security consequence was established.
- Test-only; no demonstrated product mutation or security consequence.
- The absolute inherited value must differ from the fixture's expected directory.
- Retained separately from the same-location correctness-report entry.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_05_cor:p1:F1`,
`helm_state_provisioning_05_sec:p1:F1`.

- `helm_state_provisioning_05_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `helm_state_provisioning_05_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
