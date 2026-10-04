# An update can report success while leaving the app unlaunchable

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

An update can report success while leaving the app unlaunchable.

## Details

`F37 / COR-INSTALLER-DIRECTORY-TARGET` — **definite** — `scripts/install.sh:448` — An update can report success while
leaving the app unlaunchable

An app with a recognized installation record can reach in-place update even when a destination expected to be a file is
actually a directory. The file-replacement helper uses `mv -f temporary destination`; when the destination is a
directory, `mv` places the temporary file inside it and returns success instead of replacing the directory.

For example, if `Contents/MacOS/farhelm-desktop` is a directory, the new desktop program is moved inside it. The
installer then advances the Installed record and can report a successful update, while macOS still finds a directory
where the app's executable should be. Repeating the installer does not repair that shape. The same behavior at the app's
`farhelm` forwarding-command path can leave the Terminal command broken.

Preflight every exact file destination and refuse directories before publication. Add a regression proving that a
directory at a program destination survives an actionable refusal rather than producing false success. This is separate
from symlinks in parent directories: the incorrectly shaped destination itself is enough.

Suggested bucket: high

Possible cover: none identified.

Caveats: The app must already be damaged or modified while retaining its recognized installation record. The `mv`
behavior and subsequent success path are established by inspection; the whole installer has not been run to reproduce
this case.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `cli_lifecycle p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Modified/damaged recordedbundle required; no runtime repro. independently editable helper vs
parentdirectory links.

## Filed reviewer metadata

- `cli_lifecycle p2`: confidence as filed: definite / confirmed from code and ordinary POSIX/macOS `mv` semantics; no
  runtime reproduction. Suggested bucket as filed: `high`.
