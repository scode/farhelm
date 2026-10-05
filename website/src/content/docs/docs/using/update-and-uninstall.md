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
([What survives what](/docs/how-it-works/what-survives-what/)). **what's new** in the same menu opens the list of
releases on GitHub, where each one says what changed.

You do not have to restart right away. The update also takes effect the next time Farhelm opens, whether you quit and
reopen it or your Mac reboots.

## Check for an update now

To check right away, select the **?** button at the top of the sidebar and choose **check for updates**, or choose
**update** in the menu of your Mac's own row in the host list. Either one installs a new release if there is one. Hover
over the version number to see how the check went: still checking, installing, up to date, or failed with the reason.

A check Farhelm makes by itself never shows anything unless it installs an update. If it fails, for example because your
Mac is offline, it tries again the next day.

## Turn automatic updates off

Select the gear at the top of the sidebar and untick **install updates automatically**. Farhelm then stops checking by
itself. **check for updates** and your Mac's **update** still work, and you can still update by running the installer
again, as in [Install Farhelm](/docs/get-started/install/). The red version number still appears when a newer version is
installed, however it got there.

## How updates are checked

The update check and the download both go to get.farhelm.io over HTTPS. Before installing an update, Farhelm checks that
the release is signed with one of the project's keys, and installs nothing otherwise. The signature shows that a release
comes from the project; it says nothing about whether its code was reviewed. Installing Farhelm by hand is different:
nothing can check a signature before Farhelm is on your Mac, so the install command trusts get.farhelm.io over HTTPS. If
Farhelm ever cannot verify an update, the version number shows a warning with the command to reinstall it. See the
[security model](/docs/how-it-works/security-model/) for the rest of what leaves your Mac.

## Update remote hosts, or uninstall

Your remote hosts are not updated automatically. Once Farhelm on your Mac has restarted on a new version, the host list
shows which hosts run an older version and lets you update them; see
[Manage hosts](/docs/using/manage-hosts/#update-a-host).

To remove Farhelm, see [Uninstall Farhelm](/docs/get-started/uninstall/).
