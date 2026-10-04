# Canonical uninstall paths lose embedded newline bytes

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Canonical uninstall paths lose embedded newline bytes.

## Details

`F15 / SEC-UNINSTALL-NEWLINE` — **possible** — `crates/farhelm-helm/src/provisioning/backend.rs:2367` — Canonical
uninstall paths lose embedded newline bytes

Uninstall resolves host paths with `readlink -f`, then pipes the output through `tr -d '\n'` before placing it in a
NUL-separated inspection response. This removes every newline byte, including a newline that belongs to a symlink’s
destination path, rather than just the command’s final output terminator.

The helm treats the resulting text as the canonical path used to check whether the running binary lies inside Farhelm’s
program directory and whether systemd’s loaded service file matches the file scheduled for removal. Distinct real paths
can collapse to the same newline-stripped spelling, making those ownership checks unreliable. A normally named symlink
pointing to a destination containing a newline can trigger the conversion without deliberate filesystem interference.

Preserve canonical path bytes exactly, or refuse control-containing canonical outputs. If removing an output terminator,
remove only that terminator; do not delete matching bytes throughout the path.

Suggested bucket: **highest**. No possible cover was identified. Path corruption is confirmed, but a bypass of the
data-directory containment guard and actual file or process loss have not been established. The merged report records
that audit D21’s custom-setup acceptance was not exact cover for this mechanism; the unverified destructive consequence
remains possible.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `helm_trust p1`.

Possible cover recorded during collection: none identified.

Collection caveats: State containment bypass not established; actual data/process loss unverified. D21 audit
custom-setup acceptance not exact; natural symlink can trigger without deliberate interference.

## Filed reviewer metadata

- `helm_trust p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
