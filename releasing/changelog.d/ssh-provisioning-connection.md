---
kind: added
---

Hosts whose SSH server allows only one session per connection and asks for an interactive approval (a second factor,
say) on every new login, can now be added and updated while Farhelm is connected to them, on a best-effort basis.
Adding or updating a host now goes over its own SSH connection, separate from the one Farhelm keeps open to the host, so
the two no longer compete for that single session. Farhelm cannot answer an approval prompt itself, so on such a host
you start both connections by hand before Farhelm needs them. If you already start Farhelm's connection by hand for such
a host, updating it now needs the second connection as well, even for an update the app requires after you upgrade it,
which used to work with the one connection.

On an ordinary host this means one more SSH login when a host is added or updated; if your key asks for a touch or a
confirmation on every login, expect one more prompt.
