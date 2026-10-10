# Dollar escaping changes the executable path

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Service rendering changes the executable selected by a dollar-containing path.

## Details

F94 — **definite** — `crates/farhelm-helm/src/units.rs:382` — Dollar escaping changes the executable path

Both service renderers double literal dollars in the executable filename. Systemd expands service arguments separately
from the pathname it opens, so the generated service can fail to start or execute a different existing pathname.
Separate executable quoting from argument quoting, preserve literal executable dollars, and correct the dollar-path test
oracle. This finding concerns generation, while destructive ownership read-back is a separate defect.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:152 and :178 pass the executable through systemd_arg; :382 doubles every dollar.
- crates/farhelm/src/setup.rs:664 and :677 use these renderers for setup-owned units;
  crates/farhelm-helm/src/provisioning/plan.rs:801 uses the supervisor renderer for provisioning.
- https://github.com/systemd/systemd/blob/v255/src/core/load-fragment.c#L890 unquotes and C-unescapes the executable;
  :934 resolves unit specifiers; :1048 stores the resulting path independently of argv.
- https://github.com/systemd/systemd/blob/v255/src/core/exec-invoke.c#L4739 opens command->path before :5186 expands
  command->argv. Thus /opt/$name/farhelm renders as a request to execute /opt/$$name/farhelm.
- crates/farhelm-helm/src/units.rs:781–789 explicitly assert the doubled executable spelling and validate it using the
  same incorrect dollar-collapsing reader. This oracle permits the concrete regression.
- crates/farhelm-helm/src/units.rs:152 and :178 route executable paths through systemd_arg; :382 doubles dollars.
- crates/farhelm/src/setup.rs:664 and :677 install the rendered services;
  crates/farhelm-helm/src/provisioning/plan.rs:801 renders the provisioned supervisor service.
- Systemd v255 src/core/load-fragment.c:890, :934 and :1048 store the executable separately; src/core/exec-invoke.c:4739
  opens it before argument environment expansion at :5186.
- crates/farhelm-helm/src/units.rs:781–789 validates the incorrect spelling through a matching incorrect reader.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires a dollar in the executable path; dollars confined to ordinary arguments do not establish this defect.
- No service was started. If the doubled path exists, systemd can execute that different file instead of failing.
- Requires a dollar in the executable path.
- Ordinary argument dollar escaping must remain.
- No runtime verification was performed in this independent pass.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_12_cor:p1:F1`,
`helm_state_provisioning_12_sec:p1:F1`.

- `helm_state_provisioning_12_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
- `helm_state_provisioning_12_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
