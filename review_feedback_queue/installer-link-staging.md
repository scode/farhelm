# Terminal link staging removes a colliding user file

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Terminal link staging removes a colliding user file.

## Details

`F36 / COR-INSTALLER-LINK-STAGING` — **possible** — `scripts/install.sh:1294` — Terminal link staging removes a
colliding user file

To publish the Terminal command, the installer first stages a symlink at `~/.local/bin/.farhelm-link.<installer-pid>`.
It unconditionally runs `rm -f` on that predictable path before creating the link. A pre-existing user file with the
same name is therefore deleted solely because its name matches the installer's chosen temporary path.

Allocate private link-staging space exclusively on the same filesystem, so publication can still use a rename without
deleting an occupied name. Cleanup should remove only artifacts this invocation successfully allocated.

Suggested bucket: highest

Possible cover: none identified.

Caveats: This is a new path relative to the baseline, but the process-ID filename collision is unusual and unobserved.
No ordinary-use example or runtime reproduction establishes that premise.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_trust p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Unusual/unobserved PID-filename collision, no ordinary-use example; new base path.

## Filed reviewer metadata

- `cli_trust p2`: confidence as filed: possible / likely conditional on a regular foreign
  `~/.local/bin/.farhelm-link.<installer-pid>` already existing and not being an explicitly reserved ownership
  namespace. Deletion is confirmed by code; normal occurrence is unverified. Suggested bucket as filed: highest.
