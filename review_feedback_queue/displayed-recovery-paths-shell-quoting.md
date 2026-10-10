# Displayed recovery paths are not shell quoting

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Following a recovery command could act on a different path.

## Details

F85 — **possible** — `crates/farhelm/src/uninstall/locks.rs:94–100` — Displayed recovery paths are not shell quoting

The diagnostic path formatter is inserted into an executable `rm` or `rmdir` command, but its output is not shell
quoting. Double-quoted byte escapes do not restore non-ASCII path bytes, and dollar or backtick substitutions remain
active. Following the advice could therefore select another existing target or execute substitutions; no command was run
and supported-use consequences remain conditional. Generate a genuinely shell-safe, lossless argument for executable
advice, separately from diagnostic display formatting.

## Evidence and triage context

- uninstall/locks.rs:94–100 inserts path_text into an executable rm/rmdir command. ownership.rs:409–421 produces
  diagnostic double quotes, byte escapes for non-ASCII, and leaves printable dollar/backtick syntax unchanged. :112–159
  accepts a matching native installation root without shell-metacharacter restrictions.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2081–2087 permits diagnostic rendering but forbids acting on converted paths; valid UTF-8 non-ASCII paths also
  expose the encoding mismatch. FILTER.md:36–42 excludes wrong-target destructive actions and security consequences.
  SPEC.md:2158–2164 excludes deliberately constructed same-account interference, but does not establish coverage for
  ordinary literal-dollar or non-ASCII names.

Caveats:

- No command was executed. Actual deletion outside the intended lock requires the transformed pathname to resolve to
  another existing target, or the user to execute advice containing shell substitutions. A deliberately malicious
  same-account pathname may fall outside SPEC.md:2160–2164, but that does not establish coverage of ordinary non-ASCII
  or literal-dollar paths. The original non-ASCII-only claim must not be silently broadened into a confirmed exploit.
- No command executed. Destructive impact requires the transformed path to identify another existing target or
  substitution syntax to be executed. Do not present deliberate malicious naming as a confirmed supported-use exploit.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_03_cor:p1:C6`.

- `cli_installation_03_cor:p1:C6`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.
