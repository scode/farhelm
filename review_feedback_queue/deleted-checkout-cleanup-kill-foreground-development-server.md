# Deleted-checkout cleanup can kill a foreground development server

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Preview cleanup could terminate a foreground development server.

## Details

F78 — **possible** — `website/scripts/preview.sh:151-157` — Deleted-checkout cleanup can kill a foreground development
server

When a checkout directory has disappeared, the preview script treats an Astro-shaped command line as evidence that it
may take over the listener. It does not establish that the listener is an owned background preview. A manually started
foreground server retaining that command line could therefore be stopped outside the documented contract. That
surviving-server premise was not reproduced. Require positive background-instance ownership or refuse takeover when the
distinction cannot be established.

## Evidence and triage context

- website/scripts/preview.sh:151-157 accepts a deleted cwd and matching Astro dev command line.
- website/scripts/preview.sh:163-172 immediately signals that listener without consulting background lock metadata.
- website/scripts/preview.sh:101-107 and 134-145 require background metadata on the ordinary stopping path, showing the
  missing distinction.
- website/scripts/preview.sh:186-192 reaches takeover when starting a preview while another process holds the port.
- website/AGENTS.md:49-53 explicitly excludes foreground astro dev from processes the preview script may stop.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- website/scripts/preview.sh:22-27 permits taking over abandoned previews, but website/AGENTS.md:49-53 forbids stopping
  foreground servers. TRIAGE_OUTCOMES.md:6610-6620 covers PID reuse after orphan identification, not incorrect
  foreground ownership classification. review_feedback_queue/preview-ready-identity.md:14-26 covers false readiness, not
  signaling.

Caveats:

- Requires a foreground listener retaining the matching CLI argv after its cwd is deleted; that Astro behavior was not
  reproduced.
- No file loss or lost session work was established.
- The code comment's assertion that requests hang does not establish permission to terminate the foreground process.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_11_cor:p1:F2`.

- `automation_website_11_cor:p1:F2`: confidence as filed: possible; suggested bucket as filed: highest.
