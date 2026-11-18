# Deleting a retired session can remove a replacement container

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Deleting an old experimental container session can remove someone’s later container if it uses the same Podman name.
That removes the new container’s files and running work.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R9-3.

Source references: `crates/farhelm-supervisor/src/container.rs:82–99, 171–189`.

The old session remains listed after its container is removed outside Farhelm. If a separately created container takes
the old name, the supervisor's retired runtime derives its Delete target from farhelm-<session UUID> alone. It
force-removes that name without inspecting legacy ownership and then confirms only that the name disappeared.

Sol rated this source-confirmed and critical. Opus verification R9-3 confirmed it and rated it noncritical because it
requires the retired runtime and deliberate reuse of an exact reserved name. Highest reflects the established loss of
files and processes in an unrelated container. The existing retired-container lookup-absence findings concern missing
Podman or lookup failure, rather than a successful remove of the wrong container. The retired runtime's Delete exception
does not explicitly authorize deleting later name occupants.

Inspect the candidate and require its legacy ownership labels or another positive proof before removal. Test a foreign
container with the exact retired name.
