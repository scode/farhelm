---
kind: fixed
---

Removing a host just after its setup or update finished could, rarely, be refused with a message saying the host was
busy with a setup, update or uninstall, although it had already finished. Removal now goes through as soon as the setup
or update shows as finished. The same goes for deleting a sandbox or container session's environment right after the
session started.

For the curator: the refusal could only come in a short moment between the helm showing the run as finished and letting
go of the host, so it mostly hit scripts or agents that remove a host the instant its setup ends, and machines under
heavy load. The old browser test suite retried such a removal until it went through; nothing recorded how long that
took. Adding a Tensorlake sandbox as a host (not for a session) still has the same gap at its end, and so would a session
launch whose setup had to install Farhelm instead of cloning a prepared sandbox, which shows as finished before its
session exists. The entry's sentence about environments rests on reading the code and a unit test: a session launch's
end got the same fix for every kind of environment, but no end-to-end run exercised it.
