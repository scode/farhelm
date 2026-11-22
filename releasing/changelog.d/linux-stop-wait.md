---
kind: fixed
---
Updating Farhelm on a remote Linux host no longer fails when the supervisor takes more than about 30 seconds to shut down, and uninstalling no longer fails when it takes more than about a minute; Farhelm now waits as long as systemd gives it, 90 seconds. The supervisor's systemd unit also sets that 90-second shutdown timeout explicitly, so it applies even where the system default was lowered.

Before, Farhelm gave `systemctl --user restart` (Update) 30 seconds, and uninstall's whole stop step 70 seconds shared with stopping the terminal hosts, while systemd itself waits up to 90 for the supervisor's clean shutdown. A clean shutdown is designed to take about 14 seconds, so this was rarely hit; it was found by reading the code, not reported. A host picks up the explicit timeout on its next Update. Uninstall reloads systemd after removing the unit file, so its stop uses the system default, normally also 90 seconds.
