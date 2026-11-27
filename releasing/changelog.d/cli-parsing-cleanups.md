---
kind: fixed
---

`farhelm custom-agent report-conversation --help` (or `-h`) now prints the command's help instead of nothing, and a
`farhelm agent` or `farhelm spawn` command with `--prompt-file -` and a mistyped flag now fails at once instead of
first waiting for a message on standard input.

Context for curation: the reporter still stays silent for every other invocation, `--id --help` included, as the
custom-agent contract promises; the contract page's changelog has the carve-out. Smaller visible changes from the same
cleanup: `--key` and `--title-kind` on `farhelm agent template create` and `edit` now list their choices when refused;
`--template` on a template write, and `farhelm spawn` with neither `--inherit-agent` nor a launch flag, are now usage
errors (exit status 2) rather than exit status 1; and a `--prompt-file` that cannot be read, or whose contents are refused, now exits 1 rather than 2.
