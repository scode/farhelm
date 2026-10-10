# Remote attach errors can inject terminal controls into helm logs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A remote attach refusal can manipulate the helm's log display.

## Details

F22 — **definite** — `crates/farhelm-helm/src/terminal.rs:408` — Remote attach errors can inject terminal controls into
helm logs

The supervisor supplies the message used for an attach refusal. That text becomes a display-formatted error field in the
helm's stderr without escaping terminal controls. Opening the remote terminal can therefore forge or repaint
operator-visible logs. Clipboard effects depend on terminal or log-viewer behavior; arbitrary code execution is not
established. Escape the rendered error at the logging boundary using the existing policy for peer text.

## Evidence and triage context

- crates/farhelm-helm/src/client.rs:2527–2531: the supervisor's error message becomes SupervisorError.
- crates/farhelm-helm/src/client.rs:213–218: SupervisorError Display prints that message.
- crates/farhelm-helm/src/terminal.rs:195–202,530–550: attach refusal propagates the error through serve_term.
- crates/farhelm-helm/src/terminal.rs:408: the callback logs error = %e.
- crates/farhelm/src/main.rs:1729–1737: the ordinary tracing formatter writes stderr.
- [private local path] EscapeGuard applies to message, while named fields format their supplied Debug value directly.
- [private local path] DisplayValue records through Debug, whose implementation delegates to Display.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- {"matched": null, "possible": ["SPEC_impl.md:1167–1175 requires control-normalizing peer error text wherever logged.",
  "SPEC.md:2152–2170 treats remote supervisor messages as untrusted.", "TRIAGE_OUTCOMES.md:6940–6955 covers degraded
  linger stderr at a different independently editable logging site.", "The permitted OSC 52 effect in session terminal
  content does not authorize controls through operator logs."]}

Caveats:

- No exploit executed.
- Effects depend on the terminal or later log viewer; OSC 52 requires terminal support.
- No exploit was executed.
- Repainting and clipboard effects depend on the terminal or later log viewer.
- This establishes control injection, not arbitrary code execution.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_07:p1:F1`.

- `gap_helm_connections_sec_07:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
