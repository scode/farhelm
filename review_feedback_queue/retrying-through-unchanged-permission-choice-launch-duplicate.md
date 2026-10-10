# Retrying through an unchanged permission choice can launch a duplicate session

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An unchanged permission choice can make a launch retry create a duplicate.

## Details

F103 — **definite** — `crates/farhelm-ui/src/list/create_form.rs:5647` — Retrying through an unchanged permission choice
can launch a duplicate session

A permission-choice callback clears the retained request key even when it leaves the effective launch request unchanged.
The subsequent retry consequently uses a fresh key, while the server deduplicates only by key. One intended launch can
produce a second agent in the same folder. Preserve the retained key whenever a choice callback leaves the effective
request binding unchanged, consistently across the choice buttons.

## Evidence and triage context

- crates/farhelm-ui/src/launch_controls.rs:23–39,365–366: Enter invokes the permission callback before invoking the
  primary action, including on an already-selected choice.
- crates/farhelm-ui/src/list/create_form.rs:5640–5647: the permission callback unconditionally clears intent_key. Effort
  and trust do likewise at 5632–5638 and 5649–5656; harness and launch-tab callbacks clear it at 3709 and 3719.
- crates/farhelm-ui/src/list/create_form.rs:3722 and 840–843: the choice shortcut submits the same form. Lines 4237–4255
  mint a key when none is retained; lines 4452–4462 send it.
- crates/farhelm-ui/src/list/create_form.rs:4519–4539: an ordinary ambiguous failure preserves the key, which the
  unchanged-choice callback subsequently discards.
- crates/farhelm-helm/src/sessions.rs:1618–1633 forwards intent_key;
  crates/farhelm-supervisor/src/service/core.rs:7010–7027 looks up that key and allocates a new session identity when no
  reservation exists.
- SPEC.md:601–603 promises one session or a clear error for one intended create, including timeout retries.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- TRIAGE_OUTCOMES.md:2926–2937, create-retry-duplicates-agent-without-manager.md, covers surviving detached processes
  after incomplete cleanup without systemd. It does not cover client-side replacement of an unchanged request's key.

Caveats:

- Requires the first existing-folder create to commit while its response is lost or undecodable.
- Fresh-checkout retries have separate retained-attempt handling; that path is not established as affected.
- No runtime reproduction or actual work loss was demonstrated.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_10_cor:p1:F1`.

- `ui_desktop_10_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
