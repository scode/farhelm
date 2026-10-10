# Multiple `ExecStart` commands are incorrectly treated as last-assignment-wins

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A service running commands from multiple installations could be assigned to only one.

## Details

F14 — **possible** — `crates/farhelm-helm/src/units.rs:252` — Multiple `ExecStart` commands are incorrectly treated as
last-assignment-wins

The service reader replaces each nonempty start command with the next one and reads only the first command within a
directive. Systemd can instead retain several commands in a valid one-shot service. An edited, marked unit serving two
installations could consequently be classified using incomplete ownership evidence and removed during uninstall of just
one. The supported scope of those edits remains unresolved. Track commands after the last reset, including separators,
and refuse ambiguous ownership.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:247–252 handles an empty reset but replaces effective on every subsequent nonempty
  assignment.
- Systemd v255 src/core/load-fragment.c:1051 appends parsed commands. /usr/share/man/man5/systemd.service.5.gz:529–533
  permits multiple commands for Type=oneshot and resets the list only on an empty assignment.
- crates/farhelm/src/setup.rs:501–517 recognizes ownership from the single returned pathname; :898 and :914 disable,
  stop and delete the entire unit.
- crates/farhelm-helm/src/units.rs:1038–1043 requires last-assignment-wins in a test, thereby pinning the incorrect
  behavior.
- crates/farhelm-helm/src/units.rs:252 retains only the last nonempty ExecStart; :266 reads only the first token of each
  directive.
- Systemd v255 src/core/load-fragment.c:1051 appends commands. The installed systemd.service manual at :529–533
  describes repeated directives; :1688 also permits semicolon-separated commands within one directive.
- For repeated directives naming A and B, Farhelm returns B; for a single directive containing A ; B, it returns A
  without detecting B.
- crates/farhelm/src/setup.rs:501–517 uses that incomplete result for ownership, and :898–915 stops and removes the
  entire service.
- crates/farhelm/src/uninstall.rs:301 rechecks using the same parser and :184 performs removal.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "SPEC.md:2160–2164", "comparison": "Deliberately manufacturing a misleading marked unit is excluded; the
  text does not conclusively exclude ordinary edits to a genuine setup-created unit."}
- {"basis": "SPEC.md:367; TRIAGE_OUTCOMES.md heading provisioning-ignores-unit-drop-ins.md", "comparison": "Effective
  overrides and drop-ins are a different mechanism. This finding concerns multiple commands in the base file being
  parsed."}
- {"basis": "SPEC.md:2160–2164", "comparison": "A deliberately fabricated ownership trap is excluded. An existing
  setup-owned unit edited to contain legitimate additional commands is not conclusively addressed."}
- {"basis": "SPEC.md:367", "comparison": "The exclusion of effective override analysis does not directly cover commands
  already present in the inspected base file."}

Caveats:

- Ordinary non-oneshot services with multiple commands are rejected by systemd; the working multi-command premise
  requires Type=oneshot.
- Oneshot commands run sequentially; simultaneous processes from both commands are not required to demonstrate loss of
  the shared service integration.
- Current generated units contain one command. No service was executed or removed.
- A valid multiple-ExecStart service requires Type=oneshot.
- The current templates emit one command.
- The report's definite label overstates certainty about the supported edited-unit premise; the parser mismatch itself
  is definite.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_12_cor:p1:F5`,
`helm_state_provisioning_12_sec:p1:F2`.

- `helm_state_provisioning_12_cor:p1:F5`: confidence as filed: possible; suggested bucket as filed: highest.
- `helm_state_provisioning_12_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
