# Setup writes unusable service paths when the executable path contains `$`

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Setup persistently writes the wrong service executable for dollar-containing paths.

## Details

F114 — **definite** — `crates/farhelm/src/setup.rs:664` — Setup writes unusable service paths when the executable path
contains `$`

The service argument encoder doubles literal dollars even when encoding the executable position. Systemd preserves those
dollars in the filename it opens, so a selected dollar-containing executable produces a different service pathname.
Unless that other file exists, the helm and supervisor cannot start, and retrying setup repeats the error. Encode
executables separately, preserve their literal dollars, and validate read-back against systemd rather than only the
project's inverse parser.

## Evidence and triage context

- crates/farhelm/src/setup.rs:605 canonicalizes the actual executable; :664–681 supplies that executable to both service
  renderers without rejecting dollar signs.
- crates/farhelm-helm/src/units.rs:152 and :178 encode the executable using systemd_arg; :375–382 doubles every dollar
  sign.
- crates/farhelm-helm/units/farhelm-supervisor.service.in:6 and crates/farhelm-helm/units/farhelm-helm.service.in:5
  place the encoded value in the executable position.
- crates/farhelm/src/setup.rs:750 writes the generated units; :758–764 reloads and enables them. Repeating setup
  generates the same incorrect executable spelling.
- crates/farhelm-helm/src/units.rs:301–303 also collapses doubled dollars while parsing the executable.
  crates/farhelm/src/setup.rs:501–517 uses that parser for installation ownership selection, so renderer/parser round
  trips do not establish agreement with systemd.
- Independently inspected the retained comparison script at cli_installation_02_sec/p2-dollar/check.py:4–18: it creates
  only the single-dollar executable and verifies otherwise equivalent units with systemd-analyze. Retained run
  a01cce9e-7845-4456-a3d6-2143ea9c726f/output-head.log:1–4 records literal exit 0 and doubled exit 1, explicitly naming
  the nonexistent doubled-dollar executable. Its manifest.json:132 identifies the requested frozen commit.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- TRIAGE_OUTCOMES.md:4191–4201 concerns --no-supervisor leaving existing services on different configurations. Its
  trigger and scope differ from ordinary setup corrupting both executable paths.
- TRIAGE_OUTCOMES.md:4261–4276 concerns build-tree path rejection, not dollar-sign encoding.
- review_feedback_queue/installer-directory-target.md:14–25 concerns macOS installer mv behavior when a destination is a
  directory. Similar unlaunchability does not cover Linux unit rendering.
- TODO.md:33–43 plans asynchronous session creation, unrelated to installation path rendering.

Caveats:

- No runtime tests were executed during this verification. The retained comparison validates systemd's executable-path
  interpretation, not a complete Farhelm setup invocation.
- Failure requires a dollar sign in the resolved executable path; a dollar sign only in a symlink spelling that
  canonicalization removes does not trigger it.
- No attacker-controlled executable, credential compromise, or user-work loss was established; correctness/high is the
  supported classification.
- The ownership parser has a related interpretation mismatch, but its distinct destructive consequences were not
  established here.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_02_sec:p2:F1`.

- `cli_installation_02_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: high.
