# Test authoring review

Apply this checklist to changed tests and their fixtures. A test that intentionally observes an unsettled state should
name that boundary; waiting for readiness must not remove the behavior it exists to exercise.

- Assert the fixture premise before the action or measurement that depends on it. A successful setup command alone does
  not establish the state the assertion needs. When a wait times out, report whether the fixture premise still holds.
- Wait on the named readiness oracle for the intended boundary: created, replayed, revealed or live. Obtain live bytes
  through the test's own post-replay attachment. Preserve deliberately unsettled boundary tests.
- Establish that the peer is still alive before writing to it or relying on it during teardown. Keep failure diagnostics
  inside the lifetime of the owned fixture they inspect.
- Do not carry an unintended lock or file descriptor across a spawn. Check the actual child lifetime and inherited
  resources, including failure and cancellation paths.
- Measure an owned process with the intended runner and substrate. Distinguish baseline cost from the resource growth
  being tested; unrelated process activity is not a measurement of this fixture.
- Assert an observable that distinguishes the competing mechanisms. A shared error string, absence of output or elapsed
  delay alone may not establish the cause the test claims.
- Bound polling time and retained output, and use shared polling helpers. Do not repeatedly capture megabytes to answer
  a small readiness question. Centralize legitimate polling and observation delays; explain justified test-body sleeps
  with `// sleep-ok: <why>`.
- Establish pointer and focus state before input when the test depends on them. Wait for the intended focus handoff
  rather than relying on earlier browser activity or timing.
