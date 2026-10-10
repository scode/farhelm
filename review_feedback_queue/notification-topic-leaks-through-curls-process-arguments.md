# Notification topic leaks through curl’s process arguments

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Sending a notification exposes its private topic in curl's arguments.

## Details

F69 — **definite** — `plans/AGENTS.md:168` — Notification topic leaks through curl’s process arguments

Shell substitution places the private notification topic directly in curl's command-line arguments. On platforms where
another account can inspect those arguments, it can learn the topic during a send and gain access to subsequent private
planning notifications. Keep the URL out of command-line arguments and diagnostics by supplying curl configuration
through stdin or using a helper that reads the topic internally.

## Evidence and triage context

- plans/AGENTS.md:149–153 identifies knowledge of the topic as sufficient to read notifications and requires keeping it
  private.
- plans/AGENTS.md:158–160 directs executors and the monitor to send notifications using this mechanism; :168 expands the
  topic file into curl's URL argument.
- plans/AGENTS.md:171–174 suppresses the response body but provides no protection for argv.
- plans/CLAUDE.md resolves to AGENTS.md, so these are one editable source.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2152–2158 trusts same-account processes, whereas this claim concerns another account. SPEC.md:2208–2213 limits
  its other-local-account exception to the browser UI; it does not accept disclosure of the planning notification topic.
  No matching Planned, BUGS.md, queue, or ledger coverage was found.

Caveats:

- Capture by another account depends on OS process-inspection policy and observing curl while it runs. No secret file
  was read, notification sent, or exploit executed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_06_sec:p1:F1`.

- `automation_website_06_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
