---
kind: fixed
---
Uninstalling Farhelm from a remote Mac no longer fails its first attempt with "launchd job ... is still loaded" when the supervisor is running; previously choosing uninstall again was needed. Choosing Update for a remote Mac ran the same check and is fixed the same way.

launchd's `bootout` returns before the job is gone, and Farhelm checked for the job once right after it. It now waits up to about 15 seconds for launchd to drop the job. The uninstall failure was reproduced on v0.44.0-rc.6 and the code is the same in stable v0.44.0 and every release since remote Mac support shipped in v0.38.0; the Update failure follows from the same code but was not reproduced.
