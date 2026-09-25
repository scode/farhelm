# extract_sole_member exits silently under set -e

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a machine whose tar trips this step, the installer just stops with a failure status and prints no explanation.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F17 / COR-EXTRACT-SILENT-EXIT`, tagged **possible**. Anchors and title: `scripts/install.sh:349`,
`scripts/install.sh:350`, `scripts/install.sh:351` — Under set -e, extract_sole_member exits silently instead of
printing its refusal

`extract_sole_member` pulls one named file out of a downloaded release archive and has a series of named refusals; its
comment says those refusals are "the point of this function existing at all". The installer runs under `set -eu`, and
the function is called as an ordinary command, so any failing command inside it ends the script immediately. Two lines
defeat the refusals:

- L349, `esm_type_lines=$(tar tvzf "$esm_archive" -- "$esm_member" 2>/dev/null)`: if `tar` fails, the assignment takes
  its failing status and the shell exits right there, with tar's own error discarded by `2>/dev/null`.
- L350, `esm_type_count=$(printf '%s\n' "$esm_type_lines" | grep -c .)`: `grep -c` exits 1 when it counts zero lines, so
  an empty listing also exits before L351 can print "reports 0 metadata records, expected exactly 1".

In both cases the run ends with status 1 and prints nothing, so a `curl | sh` user has no clue what went wrong.
Suggested change: give each command substitution its own failure path (`|| { printf …; exit 1; }`) and count lines in a
way that cannot fail on zero.

Restater note: the archive has already passed its SHA-256 check by this point, so reaching this path needs a malformed
release archive, a mirror (see F23), or a `tar` that lists the member in one mode but not the other. It is a diagnostics
defect, not an integrity one.
