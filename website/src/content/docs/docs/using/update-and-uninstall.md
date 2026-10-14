---
title: Update or uninstall Farhelm
description: How Farhelm keeps itself up to date on your Mac, and how to update it yourself or remove it.
sidebar:
  order: 7
---

## Farhelm updates itself

The Farhelm app on your Mac keeps itself up to date. Shortly after you open it, and about once a day after that, it
checks get.farhelm.io for a new release. When there is one, it installs it in the background, with the same installer
you used to [install Farhelm](/docs/get-started/install/). Nothing changes on screen while that happens, and your
sessions keep running.

Only stable releases are installed this way, never a prerelease. This applies to the app installed in `~/Applications`;
a copy of Farhelm you built yourself never updates itself.

## When an update is ready

When a newer version is installed than the one you are running, the version number at the top of the sidebar turns red,
with an up-arrow in front of it. Hover over it to see which version is waiting.

Select the version number and choose **restart to update**: Farhelm quits and opens again on the new version. Your
sessions keep running while it does, as they do whenever you quit Farhelm
([What survives what](/docs/how-it-works/what-survives-what/)). **what's new** in the same menu opens the
[release notes](/docs/release-notes/), which say what changed in each release.

You do not have to restart right away. The update also takes effect the next time Farhelm opens, whether you quit and
reopen it or your Mac reboots.

## Check for an update now

To check right away, select the version number at the top of the sidebar. You can also select the **?** button there and
choose **check for updates**, or choose **update** in the menu of your Mac's own row in the host list. Each one installs
a new release if there is one. Beside the version number the app shows **checking…**, then **installing…**, **up to
date**, or **check failed**. **up to date** stays for about five seconds; **check failed** stays until you dismiss it or
check again, and hovering over it gives the reason.

A check Farhelm makes by itself shows nothing while it goes well; when it installs an update, the version number turns
red. If downloading or installing an update fails, **check failed** appears right away. If Farhelm cannot reach the
update server, for example because your Mac is offline, it tries again the next day, and shows **check failed** only
once that has kept failing for three days. A failure shown this way stays until a check succeeds, a newer version is
installed, or you dismiss it.

## Turn automatic updates off

Select the gear at the top of the sidebar and turn off **install updates automatically**. Farhelm then stops checking by
itself. **check for updates** and your Mac's **update** still work, and you can still update by running the installer
again, as in [Install Farhelm](/docs/get-started/install/). The red version number still appears when a newer version is
installed, however it got there.

## How updates are checked

The update check and the download both go to get.farhelm.io over HTTPS, which hands the release files themselves over
from GitHub, so your Mac needs to reach both. Farhelm trusts the certificates your Mac trusts and follows its proxy
settings, so this also works behind a company proxy that uses its own certificate. Before installing an update, Farhelm
checks that the release is signed with one of the project's keys, and installs nothing otherwise. The signature shows
that a release comes from the project; it says nothing about whether its code was reviewed. Installing Farhelm by hand
is different: nothing can check a signature before Farhelm is on your Mac, so the install command trusts get.farhelm.io
over HTTPS. If Farhelm ever cannot verify an update, the version number shows a warning with the command to reinstall
it. See the [security model](/docs/how-it-works/security-model/) for the rest of what leaves your Mac.

## Update remote hosts, or uninstall

Your remote hosts are not updated automatically. Once Farhelm on your Mac has restarted on a new version, the host list
shows which hosts run an older version and lets you update them; see
[Manage hosts](/docs/using/manage-hosts/#update-a-host).

To remove Farhelm, see [Uninstall Farhelm](/docs/get-started/uninstall/).
