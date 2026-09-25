# The app bundle's Info.plist is written with the caller's umask

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a shared Mac where the user installs with a group-writable umask, another local account can alter Farhelm.app so it
runs that account's code as the user the next time they open Farhelm.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F24 / SEC-INFO-PLIST-UMASK`, tagged **definite**. Anchors and title: `scripts/install.sh:1225`,
`scripts/install.sh:1218`, `scripts/install.sh:1265-1267` — install.sh writes the app bundle's Info.plist with the
caller's umask, unlike every other bundle file

The macOS bundle step sets explicit permissions on everything it creates except one file. Directories are created under
`(umask 022; …)` (L1218, L1265), both executables are `chmod 0755`, the icon `0644`, and the receipt `0600`. But
`Contents/Info.plist` is written with `cat > … <<PLIST_EOF` (L1225), outside any umask subshell and never chmodded, so
its mode is `0666` minus the caller's umask. The reviewer ran this under `umask 002` in a `mktemp -d` and got mode 664
(group-writable). The script's own comment at L906–914 plans for a permissive umask for directories but misses this
file. The bundle, with that mode intact, is then moved into `~/Applications` (L1266–1267).

Why this is a cross-account problem: on macOS, local accounts are normally all in group `staff`, the installer creates
`~/Applications` as 0755, and home directories are traversable by that group. So with a group-writable umask, another
local account can edit `Farhelm.app/Contents/Info.plist`. Launch Services honours the `LSEnvironment` key there, which
can set `DYLD_INSERT_LIBRARIES` for the unsigned, non-hardened `farhelm-desktop` (loading the other account's code into
it) or set `FARHELM_*` variables. The next time the victim opens Farhelm, that code runs as the victim. That crosses the
Unix-account boundary SPEC.md "Local authority and trust between hosts" names as the security boundary. Uninstall's
digest check would notice the edit, but only at removal time.

Suggested change: write `Info.plist` under `umask 022` or `chmod 0644` it before hashing; better, run the whole
bundle-staging block under `umask 022`.

Restater note: requires a non-default permissive umask (such as 002) at install time and a second local account on the
Mac; default macOS umask 022 is not affected.
