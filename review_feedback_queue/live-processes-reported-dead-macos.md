# Live processes are reported dead on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

macOS process checks can certify death without observing the process.

## Details

F132 — **possible** — `crates/farhelm/tests/e2e/harness.rs:2048`; `crates/farhelm/tests/e2e/harness.rs:2047` — Live
processes are reported dead on macOS

The shared death helper relies on `/proc`, which ordinary macOS does not provide. Observation failure is therefore
interpreted as disappearance: live-process assertions fail, while death assertions can pass for living processes. No
native macOS run was performed, and this does not establish a product cleanup failure. Use a platform-capable process
oracle and distinguish unavailable observation from confirmed disappearance.

## Evidence and triage context

- harness.rs:2047–2057 reads only /proc/<pid>/stat and treats failed reads as gone. :2064–2068 also uses that answer for
  disappearance. restart_with_resume.rs:664–666 and session_lifecycle.rs:5446–5450 require live processes to return
  false. main.rs:39,52 and the test macro at farhelm-testtrace-macros/src/lib.rs:55–70 add no platform exclusion.
- harness.rs:2047–2057 treats unavailable /proc observations as death. session_lifecycle.rs:5446–5450 and
  restart_with_resume.rs:664–666 assert survival using this helper, with no platform exclusion in e2e/main.rs or the
  test macro.
- crates/farhelm/tests/e2e/harness.rs:2048 treats failed Linux procfs reads as death.
- crates/farhelm/tests/e2e/harness.rs:2168 treats unavailable procfs as an empty child table.
- crates/farhelm/tests/e2e/restart_with_resume.rs:665 asserts these processes remain alive; :571 requires grandchild
  discovery.
- crates/farhelm/tests/e2e/session_lifecycle.rs:4453 independently requires grandchild discovery.
- crates/farhelm/tests/e2e/main.rs:39 and :52 include these modules without Linux gating.
- harness.rs:2047-2049 treats unavailable /proc as death; :2544-2548 returns no marked PIDs when /proc is absent.
  session_lifecycle.rs:5496-5497 and :5550 use the death helper without Linux gating. fake_agent.rs:2655 additionally
  invokes setsid, so some macOS setups may fail earlier; that does not validate the death oracle.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2175–2182 explicitly identifies macOS's alternative process observation backend. birth-oracle.md:11–25
  concerns filesystem creation-time capability, not process liveness. No exact coverage found.
- SPEC_impl.md:2175–2182 confirms macOS requires another process backend. birth-oracle.md concerns a different
  filesystem oracle and does not cover this helper.
- No exact existing coverage. birth-oracle.md is a different filesystem-capability oracle.

Caveats:

- No native macOS execution. The failure follows from the unsupported /proc path.
- No native macOS execution. Same location as cli_installation_05_sec:p1:F1; no shipped lifecycle failure is
  established.
- No native macOS execution. This is a test-oracle defect, not evidence that the refused product operation actually
  kills the process.
- No native execution. Same-location duplicate of cli_installation_05_cor:p1:F1; the product's refusal behavior is not
  shown wrong.
- No macOS execution.
- Some daemon fixtures additionally require setsid; that can cause an earlier setup failure but does not repair the
  shared observer.
- This establishes test defects, not a current production cleanup failure.
- Aggregate needs two independently editable findings: process_is_gone/death observation, and children_of/child
  discovery.
- Some reparent fixtures can fail earlier due to setsid availability.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F1`,
`cli_installation_05_sec:p1:F1`, `cli_installation_07_sec:p1:F3`, `cli_installation_08_cor:p1:C7`.

- `cli_installation_05_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_05_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_07_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_08_cor:p1:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
