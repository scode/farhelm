# Probe cleanup timeout discards a parsed remote path

Reviewed commit: 3bb35222ce3fc4bcc2a6c2aed16475351e9d208f

## TLDR

A remote host that answers correctly can still be refused during discovery or update planning if an escaped SSH helper
keeps its diagnostic pipe open. Cleanup discards an executable path already received from that host.

## Details

Suggested bucket: **other**. Confidence: **confirmed conditional source mechanism**; no real helper configuration or
ordinary OpenSSH trigger was demonstrated. No unsafe installation, wrong-target action or user-work loss is established.

`crates/farhelm-helm/src/provisioning/backend.rs:1852–1855` replaces all accumulated diagnostics with the default on the
two-second cleanup deadline. The drainer keeps complete resolved-path records only in its task-local result until EOF
(`1490–1527`), while successful and skewed hello branches require that path (`1918–1935,1979–1996`). A helper closing
stderr after three seconds would previously permit completion but now loses that parsed metadata.

Discovery and update planning propagate this refusal before registration or installation planning
(`provisioning/service.rs:545–550,603–618,1008–1012`); confirmation revalidation does likewise (`1683,1694`). The permit
is released. This is distinct from the separately queued planning-pool drain hang, which the deadline improves. Tests at
`backend.rs:3091–3132` cover an empty open stream and completed diagnostics, not a complete control record followed by
an open stream. Preserve parsed control metadata independently of EOF if this case is to succeed.

Possible cover: `TRIAGE_OUTCOMES.md:16607–16624` authorizes the bounded join, without expressly deciding loss of parsed
records; `SPEC_impl.md:4455–4458` retains resolved coordinates. The rare safe-retry filter could cover a transient
incident, but does not clearly cover a configured helper that makes every attempt fail. No exact duplicate or other
coverage was found. Proposed drop: none; retain the unverified configuration premise.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g055/f1`.
