# Unsupported C escapes become a different executable instead of refusing classification

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Escaped service paths could make uninstall choose the wrong service.

## Details

F12 — **possible** — `crates/farhelm-helm/src/units.rs:296` — Unsupported C escapes become a different executable
instead of refusing classification

The service reader drops the backslash from escape sequences instead of decoding or refusing them. For example,
systemd's `\x20` denotes a space, while Farhelm turns it into literal `x20`. In an edited, marked service unit, that
invented path could select another installation's service or leave the intended service pointing at a removed
executable. Whether such edits are within supported scope remains unresolved. Decode the supported grammar correctly or
reject unsupported escapes before classifying ownership.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:296 consumes any backslash and appends only its next character. A literal
  /opt/a\x20b/farhelm therefore becomes /opt/ax20b/farhelm.
- Systemd v255 src/core/load-fragment.c:890 applies EXTRACT_CUNESCAPE to the executable.
  /usr/share/man/man7/systemd.syntax.7.gz:274–276 defines \x followed by two hexadecimal digits as the corresponding
  character.
- crates/farhelm/src/setup.rs:501–517 treats the invented pathname as ownership evidence; :334–335 selects a matching
  service; :898 and :914 stop and delete it.
- crates/farhelm-helm/src/units.rs:273–277 promises unsupported C-style spellings will be unrecognized, but :296 does
  not implement that refusal.
- crates/farhelm-helm/src/units.rs:296 turns the literal /opt/space\x20install/farhelm into
  /opt/spacex20install/farhelm.
- Systemd v255 src/core/load-fragment.c:890 C-unescapes the executable; systemd.syntax manual :274–276 defines the
  hexadecimal escape, producing /opt/space install/farhelm.
- crates/farhelm/src/setup.rs:498–517 checks the marker and compares the incorrectly decoded path with the installation
  selected for uninstall.
- A false positive enters selected at crates/farhelm/src/setup.rs:335 and reaches disable --now at :898 and deletion at
  :914.
- A false negative enters retained_paths at crates/farhelm/src/setup.rs:336; crates/farhelm/src/uninstall.rs:184–186 can
  then retain that integration while removing the selected installation.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "SPEC.md:2160–2164; TRIAGE_OUTCOMES.md heading installer-malformed-record.md, lines 6759–6764",
  "comparison": "These exclude deliberately crafted filesystem interference. The finding can instead concern ordinary
  re-quoting of a genuine setup-created unit; the cited examples do not conclusively classify that trigger."}
- {"basis": "SPEC.md:363–367", "comparison": "Operator-authored services and effective overrides are outside removal
  authority, but the report concerns parsing the base file of an existing setup-marked service."}
- {"basis": "SPEC.md:2160–2164; TRIAGE_OUTCOMES.md:6759–6764", "comparison": "The exclusion covers deliberately crafted
  filesystem states. It does not unambiguously classify ordinary alternative quoting in a genuine setup-marked
  service."}

Caveats:

- The current renderer does not emit \x20.
- Wrong-service removal additionally requires the incorrectly decoded path to match the selected installation.
- No destructive reproduction; the unresolved premise concerns supported scope, not whether the parser drops the
  backslash.
- Requires non-renderer escape syntax in a marked base unit.
- Wrong-service deletion needs a matching incorrectly decoded installation path.
- No destructive reproduction. The confidence downgrade concerns scope and reachability in ordinary supported use, not
  the decoding mismatch.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_12_cor:p1:F3`,
`helm_state_provisioning_12_sec:p1:F3`.

- `helm_state_provisioning_12_cor:p1:F3`: confidence as filed: possible; suggested bucket as filed: highest.
- `helm_state_provisioning_12_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
