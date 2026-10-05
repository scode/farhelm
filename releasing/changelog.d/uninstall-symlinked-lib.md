---
kind: fixed
---

Uninstalling Farhelm from a remote host in the hosts list no longer reports success when the host's
`~/.local/lib/farhelm` is a symbolic link to a directory elsewhere. Before, uninstall deleted only the link, said it had
finished and dropped the host from the list, while the Farhelm program it pointed to stayed on the host. Uninstall now
refuses such a host before changing anything, names where the link points, and keeps the host listed. Removing the host
from the list without uninstalling still works for it, as for any host.
