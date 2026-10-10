# Emergency browser cleanup treats command-line text as process ownership

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Emergency cleanup can kill processes merely mentioning a test path.

## Details

F77 — **definite** — `scripts/test-start-stack-cleanup.sh:265–266`; `scripts/test-start-stack-cleanup.sh:266` —
Emergency browser cleanup treats command-line text as process ownership

The harness selects processes for SIGKILL by matching the fixture state path in command-line text. A diagnostic or
editing process can mention that path without belonging to the harness, so test failure cleanup can kill work it never
created. Neither PID reuse nor malicious interference is needed for this ownership error. Use owned process instances or
containment; escaping the matching expression does not establish ownership.

## Evidence and triage context

- scripts/test-start-stack-cleanup.sh:265 converts only the final character into a bracket expression; :266 runs pkill
  -KILL -f on that regex. A same-account tail or editor whose argv names a log under the state directory matches.
  Failure callers at :314, :331 and :353 reach this site. The read-only matcher at :179 escapes metacharacters, but
  emergency_cleanup does not.
- scripts/test-start-stack-cleanup.sh:177-180 matches every command line containing the state path, excluding only mux
  entries.
- scripts/test-start-stack-cleanup.sh:194-203 treats any such remaining observer as cleanup failure.
- scripts/test-start-stack-cleanup.sh:261-266 converts the state path into a pkill -f pattern and sends SIGKILL to every
  match.
- scripts/test-start-stack-cleanup.sh:313-314,330-331,352-353 invoke that destructive cleanup after failed phase checks.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2152–2164's deliberate same-account interference acceptance does not cover accidentally killing an ordinary
  diagnostic process. TODO.md:241–253 covers lingering stacks, not this kill set.
- TODO.md:241-252 concerns fixture processes surviving parent termination, not killing unrelated observers; it is also
  in Deflake, not Planned. FILTER.md:36-42 excludes process/work loss and wrong-target actions.

Caveats:

- Requires emergency cleanup and an unrelated matching command line. No signals were sent.
- No runtime reproduction.
- An independently launched observer must still reference the path in its command line.
- The claim does not depend on the additional unescaped-regex weakness.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F7`,
`automation_website_10_sec:p1:F1`.

- `automation_website_10_cor:p1:F7`: confidence as filed: definite; suggested bucket as filed: highest.
- `automation_website_10_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
