# Saving and restoring cursor attributes loses extended formatting

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Cursor restore leaves extended formatting changed after the save.

## Details

F254 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.saveCursor/restoreCursor, R4285-4290` —
Saving and restoring cursor attributes loses extended formatting

Saving cursor attributes retains flags referring to extended formatting but not the extended state itself. Changing
underline style after the save can therefore survive restoration, altering later output despite the intended temporary
drawing change. Seek an upstream consistent snapshot of attributes and their extended formatting, preserving the policy
that the bundled vendor bytes remain unchanged.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R3589-3592: ESC 7 and ESC 8 register saveCursor and restoreCursor.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R4286-4289: only fg/bg are saved and restored; extended is untouched.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R4200-4211: selecting an underline style clones and updates
  extended.underlineStyle, and sets the associated flags.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R4818: underline style is subsequently read from extended when the
  restored flags indicate extended attributes.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- FILTER.md:24-34 requires a rare trigger as well as a qualifying consequence. A normal save/change/restore sequence is
  not demonstrated to satisfy that trigger requirement.

Caveats:

- Confirmed narrowly for underline style; this does not establish identical save/restore requirements for every extended
  field.
- The state persists until another relevant formatting command or reset, rather than necessarily forever.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F6`.

- `vendor_02_cor:p3:F6`: confidence as filed: definite; suggested bucket as filed: other.
