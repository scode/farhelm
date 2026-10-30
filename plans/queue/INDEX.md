# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `podman-local-image.md` — Containerized launches name a local Podman image instead of an OCI archive file, so a new container starts in about half a second instead of about twelve; archive settings in old templates and sessions read as an empty image name.
- [pending] `container-host-account.md` — Containerized sessions run as an account matching the host user's name and uid, chosen from the image (the owner of the host home path if the image has it, then the uid, else a new account seeded from /etc/skel), instead of uid 1000. (after `podman-local-image.md`)
- [in-flight 5e756c] `typed-quick-launch.md` — Template names can be typed inside the Cmd+X quick-launch sequence (unique prefix, Tab completion, a candidate list under the pill); new template names can no longer contain spaces.
