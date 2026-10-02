---
kind: fixed
---

Adding a host now refuses a remote farhelm path or remote state directory that starts with `~` or is otherwise not an
absolute path, and says to give an absolute path instead. Farhelm never expanded `~` in those fields, so
`~/.local/bin/farhelm` made a host where Farhelm was installed look as if it was not, and offered to set it up again,
and `~/state` pointed at a folder literally named `~`. A bare program name such as `farhelm`, found through the host's
`PATH`, still works for the remote farhelm path. Hosts already registered with such a path keep working as before, and
running their set up again repairs them. An `--ensure-hosts` file now has to give absolute paths for a host it adds; for
a host already registered, a path the rule refuses only makes the helm log a warning naming it at startup.
