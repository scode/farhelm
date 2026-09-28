---
kind: fixed
---

Setting up or updating a host registered by IPv6 address (such as `fe80::1`) or as an `ssh://user@host:port` address now works. The upload of the Farhelm binary used a separate `sftp` step that read those addresses as a different host, so it always failed, and in rare setups could send the binary to a machine you never registered. Uploads now go over the same `ssh` connection as every other step, and hosts no longer need the sftp subsystem.
