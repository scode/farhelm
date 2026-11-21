# Settings can show automatic updates on while the updater has paused them

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Settings can still show automatic app updates as on when the updater has paused them because preferences are unreadable.

## Details

2708e534fb298446095a3be9e0759c11174d34f9 changes a read or parse error to off in updater.rs:476–482. The scheduler
rereads the file before a due automatic check (951–960), and the check rereads it before install admission (1342–1345).
The UI caches one effective read at startup and only changes it on a successful toggle. The reviewed commit's
desktop/updates.rs:96,105,158–171 shows this arrangement, and the pinned tip still has it at 100–114,163–187. Later
worker readouts only replace the stored readout, with no automatic-policy field to update the switch. Settings uses that
cached value both to check the switch and to describe whether background checks and installs occur
(settings/app.rs:48–54,136–150). The premise is a window that initially read on, followed by a file read/parse error
during an automatic policy check. Publish the worker's effective policy or a distinct paused/unreadable status to
Settings, retaining the user's saved choice and the current fail-closed behavior. Keep the normal on-demand route
available.

Confidence: confirmed by inspection for the stated read/parse error premise.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `upgradeBackend/l10`.
