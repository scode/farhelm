# Light-theme documentation headings have unreadably low contrast

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Light-theme section headings have insufficient contrast.

## Details

F275 — **definite** — `website/src/styles/farhelm.css:219` — Light-theme documentation headings have unreadably low
contrast

The light theme changes the page background but leaves third- and fourth-level heading text pale cyan. Their
foreground/background contrast becomes very low, making documentation sections and release-note categories difficult to
identify. Use a theme-aware heading foreground or override that heading token in light mode. This concerns executable
theme CSS rather than documentation wording.

## Evidence and triage context

- website/src/styles/farhelm.css:81 defines --farhelm-cyan as #a9bfd9.
- website/src/styles/farhelm.css:108-127 sets the light background to #f8f9fb without overriding that foreground token.
- website/src/styles/farhelm.css:217-222 overrides the theme-aware general heading rule with the pale token for h3 and
  h4.
- website/astro.config.mjs:70 loads this stylesheet.
- website/src/content/docs/docs/using/session-list.mdx:60 contains an affected h3;
  website/scripts/release-notes.mjs:128-130 generates affected h4 category headings.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No browser rendering was performed.
- The declared color pair has approximately 1.79:1 contrast; readability impact varies by reader and display.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_11_cor:p1:F1`.

- `automation_website_11_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
