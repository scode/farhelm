# Setup’s sign-in command ignores its selected state directory

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Setup's sign-in advice can target a different helm.

## Details

F128 — **definite** — `crates/farhelm/src/setup.rs:813` — Setup’s sign-in command ignores its selected state directory

After setup selects a custom state directory, the printed sign-in command omits that directory. Following it can read
another helm's token or create a database in the default location instead of authenticating to the configured helm.
Include the resolved state directory as a correctly shell-quoted `--state-dir` argument in the advice.

## Evidence and triage context

- setup.rs:636–639 resolves the selected directory and :661–681 pins it into both service definitions, but :811–813
  prints an unqualified token-show command. main.rs:835–840 and :1097–1098 pass an omitted directory through to
  token_control.rs:491–492, which selects the default. token_control.rs:212–218 creates/opens that directory and shows
  or mints its credential.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- token-recovery-path.md:11–25 requires non-UTF-8 input and an uncertain rotation result at token_control.rs; it does
  not cover ordinary setup or setup.rs:813. FILTER.md:24–42 does not cover this ordinary trigger and wrong-target
  persistent mutation. No matching Planned item or BUGS.md entry.

Caveats:

- The actual helm credential remains intact.
- Requires the custom setup directory to differ from the subsequent command's environment-derived default.
- No runtime reproduction.
- Requires the selected directory to differ from the later command's default. The configured helm's credential remains
  intact.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_02_cor:p1:F2`.

- `cli_installation_02_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
