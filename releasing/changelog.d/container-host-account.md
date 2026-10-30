---
kind: breaking
---

New containers for containerized sessions now use an account matching your host user name, user ID and home where the
image permits it, instead of the image's user-1000 account. Images prepared only for that account, as v0.32.0's
instructions asked, may need agents installed system-wide, configuration in `/etc/skel`, or an account prepared for
your host user: a newly created account does not inherit the old account's programs, configuration or `sudo` access.
Existing containers keep their account until replaced.

For curation: the plan treated containerized sessions (marked experimental in v0.32.0) as unreleased and asked for a
non-breaking entry; the landing made it breaking. Host accounts whose user or group ID is too large for rootless Podman
to map (common with directory services) cannot get a container unless the image already has the user's name or user ID
and the user's home path there, owned by an ordinary account.
