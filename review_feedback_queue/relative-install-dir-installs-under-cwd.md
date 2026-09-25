# A relative or ~ FARHELM_INSTALL_DIR installs under cwd

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A user who quotes ~ in FARHELM_INSTALL_DIR gets Farhelm installed into a new ./~ directory, with messages that look like
they confirm the home directory.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F14 / COR-RELATIVE-INSTALL-DIR`, tagged **possible**. Anchors and title: `scripts/install.sh:905`,
`scripts/install.sh:917` — A relative or ~-prefixed FARHELM_INSTALL_DIR installs under the current directory

The installer uses `FARHELM_INSTALL_DIR` exactly as given (L905) and runs `mkdir -p` on it (L917). A shell expands `~`
only when it is unquoted at the start of a word, so a user who writes `FARHELM_INSTALL_DIR='~/bin'` or
`FARHELM_INSTALL_DIR="~/bin"` gets a directory literally named `~` created in whatever directory `curl | sh` ran in,
with Farhelm installed at `./~/bin/farhelm`. The success message and the PATH hint repeat the relative spelling
(`export PATH='~/bin':$PATH`), so they look like confirmation that the home directory was used. The ownership record
stores the canonical absolute path, so uninstall still works.

SPEC.md says only that the variable "selects another directory". A stray directory named `~` is a known hazard: a later
`rm -rf ~` typed to clean it up deletes the home directory instead. Suggested change: refuse any value that does not
start with `/`, and name an unexpanded leading `~` explicitly in the message.
