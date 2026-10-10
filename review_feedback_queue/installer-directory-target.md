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

## Additional finding from whole-repository collection

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

An update could redirect a file replacement into an external directory.

F97 — **possible** — `scripts/install.sh:451` — A file destination linked to a directory redirects update writes outside
the app

If a recognized app's expected file is a symlink to a writable directory, replacement can move the temporary file into
that directory rather than repair the app destination. The installer can still report success. Overwriting unrelated
content additionally requires a collision with the temporary basename, which includes the installer PID; no ordinary
product trigger was established. Preflight exact file destinations and refuse directories, including directory-valued
symlinks, before publishing updates.

## Evidence and triage context

- scripts/install.sh:1182 checks parent folders but not exact file destinations.
- scripts/install.sh:451 uses mv with a destination that can resolve to a directory.
- review_feedback_queue/installer-directory-target.md:14 already records the same file-destination directory
  interpretation and preflight remedy.
- TRIAGE_OUTCOMES.md:6822 explicitly retained analogous symlink redirection despite the deliberate-state exclusion.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- Existing installer-directory-target.md covers the underlying mechanism and false success. SPEC.md:2160 excludes
  deliberately crafted same-account states, but the explicit analogous installer-symlink decision makes outright
  exclusion inappropriate.

Caveats:

- Requires an existing recognized bundle with a modified file destination and a writable directory behind its symlink.
- Overwriting unrelated content additionally requires an existing entry matching the temporary basename, which includes
  the installer PID.
- The mv consequence is established by inspection; native macOS reproduction and an ordinary product-generated trigger
  were not established.
- No ordinary product-generated trigger or native macOS reproduction established.
- Unrelated-file overwrite additionally requires a colliding temporary basename containing the installer PID; do not
  present data loss as established.
- The exact destination and its external-write consequence should remain explicit in the extension.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_10_sec:p1:F2`.

- `cli_installation_10_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
