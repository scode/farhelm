### What this was about

The release test that provisions a CentOS host temporarily edits the operator's SSH configuration. Installing its test
alias, removing it afterwards, and clearing an old alias all rewrote the live file in place. An interruption or failed
write could erase unrelated SSH settings. Cleanup could also continue after a failed content-generation step and publish
incomplete content.

The maintainer accepted the usual minor complexity of atomic replacement. This plan carries out all three recorded
SSH-config outcomes in one draft PR.

### Things you should know

Each edit now prepares complete content and publishes it by renaming a temporary file beside the config's resolved
target. A failed generation, copy, permission change or rename leaves the existing configuration untouched. The target's
mode is preserved, and a symlinked config keeps its symlink. Cleanup reports a refused removal and continues with its
other work.

Replacement changes the target's inode, the accepted cost of avoiding partial writes. An interruption before publication
may leave a recognisable temporary file beside the target. The script's existing advisory lock still prevents its
concurrent runs from dropping each other's aliases. This is atomic publication, with no added power-loss durability
guarantee or coordination with writers outside that lock.

### Open questions and possible follow-ups

None. All three outcomes fit the agreed small helper; none was dropped by the complexity gate. This changes the release
test's safety, with no shipped Farhelm behavior change.

### PRs

- [#1805 — protect SSH config during CentOS provisioning](https://github.com/scode/farhelm/pull/1805/changes), one draft
  PR for all three outcomes.

### Checks run, reused and skipped

- `bash -n scripts/test-provision-centos.sh` and `shellcheck scripts/test-provision-centos.sh` passed. They inspect the
  changed shell code.
- The private proof extracted the actual helper and exercised 16 direct-file, symlink, mode,
  generation/copy/permission/rename failure and alias-removal scenarios. All passed, recorder
  `09e1b46e-39d3-43ad-a6cc-a483d24b1451`. It checked preserved bytes on refusal, including conditional calls that
  disable errexit.
- `bash scripts/test-provision-centos.sh` passed against CentOS Stream 9 with static payloads and pinned tmux, outer
  recorder `33b52df1-4da6-45fd-83f4-5d0f80c6307a`. Its focused install/update/session/uninstall test passed 1/1 in 181
  seconds, recorder `8da630eb-d604-4a58-ab98-6d703e7485c8`; retained output contains no runtime skip.
- Those runtime checks covered the script at pre-rebase revision `51a4d4fd`. They are reused after rebasing onto
  `6d6c25f6209e`: the script is unchanged, and inspection of the intervening sweep, sound, download and release-note
  changes found no interaction with its config edits or provisioning paths. The queue-only changes also do not affect
  execution.
- `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed again after adding the PR URL. All three
  ledger fields are complete and their feedback files and full index entries are removed.
- Broader Rust and browser suites, Clippy, the source-sleep checker and changelog checks were skipped: the diff changes
  a release-test shell script and bookkeeping, with no Rust/browser tests, shipped behavior or changelog fragment. The
  focused failure proof and real CentOS run cover the concrete changed behavior.

### Review gate outcome

The required fresh-context gpt-6.1-sol high source review passed with no remaining correctness, design, Bash or
test-quality findings. It inspected all three publication paths, the private failure proof, and the full test-authoring
contract. It did not execute runtime checks; the executing session ran those separately. A second fresh gpt-6.1-sol
medium wording reader understood the failure and inode tradeoff and found no unclear or contradicted claims.
