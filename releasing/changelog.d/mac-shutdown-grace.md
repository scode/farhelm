---
kind: changed
---
On a remote mac host, we now configure launchd with a 60 second shutdown timeout to ensure there is (in practice) always enough time for clean shutdown on uninstall or upgrades.

The maintainer's requested wording said 90 seconds, to match systemd's default on Linux; launchd records any `ExitTimeOut` above 60 as 60 (checked on macOS 26 and 27), so the entry says 60. launchd's default had been about five seconds, which cut the supervisor's orderly stop (closing viewers' streams and checkpointing terminals, up to about 14 seconds by design) short with a SIGKILL. A host set up before this change keeps the old timeout until its next Update, and that Update itself still stops the old supervisor under the old timeout.
