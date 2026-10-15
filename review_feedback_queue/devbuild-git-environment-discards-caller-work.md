# Devbuild can reset another working copy through inherited Git selectors

Reviewed commit: ce3e52cd05746f28fc0bac78606e53358944dcc4

## TLDR

Running the Mac development-build helper with Git repository selectors in its environment can overwrite edits and delete
untracked files in another working copy.

## Details

Suggested bucket: `highest`, for destructive changes to work outside the helper's owned checkout. Confidence: confirmed
for the destructive command sequence using two isolated scratch repositories; the complete networked Mac build was not
run. Source: separate adversarial trailing-review agent, desktop and devbuild scope. Proposed drop: none.

The introducing commit's `scripts/devbuild-github.sh:122,135–138` inherits `GIT_DIR`, `GIT_WORK_TREE`, and related Git
selectors in both its origin check and its force checkout/clean. `git -C` does not override those selectors. If they
select another Farhelm clone, the origin guard accepts that clone and the reset and cleanup discard its tracked edits
and untracked files. The intended development checkout need not change at all.

The isolated reproduction ran the same origin check, force checkout, and clean with selectors naming a scratch victim.
The guard returned the accepted repository URL, the victim's edited tracked file became the target commit's contents,
and its untracked file disappeared; the intended owned checkout stayed unchanged. No real working copy was modified.

This persists at `39207c0c6369b2b223fe106e4f2d721f3c259074`, where the relevant commands are at
`scripts/devbuild-github.sh:128,141–144`, and at pinned tip `75af240c09c7ecd5b81a0fcba9e03f204b0be0a7`. The added
symlink refusal fixes a different redirect. Scrub repository-selecting and injected Git configuration variables for
every Git child before inspecting ownership or mutating the checkout. A regression should establish that a foreign
repository's tracked edits and untracked files survive the helper's entire command sequence.

Possible cover: `browser-git-fixture-can-change-caller-repository.md`,
`git-discovery-fixtures-inherit-bare-git-config.md`, and the ledger's
`git-fixture-setup-modify-delete-callers-repository.md` concern test fixtures. They do not cover this helper or its
destructive reset. No matching Planned item, accepted specification behavior, BUGS entry, or filter was found.

All six assigned commits' complete patches were read, including transient versions of the helper. Runtime evidence was
limited to isolated Git and Linux MIME probes; no real Mac build or installation was changed.
