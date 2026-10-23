---
kind: fixed
---

A localhost tunnel now always listens on a port your Mac picks, never the remote service's port number. Before, a
remote service on the browser helm's port could appear at the browser helm's address on your Mac and read the sign-in
your browser keeps for it, and a Mac program expecting a port could reach the remote host instead. A remote app that
puts its own `localhost` address and port into links or redirects no longer reaches the tunnel that way; open it
through the tunnel's link instead.
