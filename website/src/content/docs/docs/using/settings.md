---
title: Settings
description: What each section of Settings holds, and which page covers it.
sidebar:
  order: 8.5
---

The gear at the foot of the [navigation rail](/docs/using/manage-hosts/), the column of icons down the left edge of the
sidebar, opens Settings at **Sessions**. Choose a section in its list to see its settings. Most take effect as soon as
you change them. Fields you have not saved keep what you typed while you switch sections; closing Settings discards
them, except that an edited recipe asks whether to save, discard or keep editing.

Type part of a section's name, a setting's name or its description in **Search** to look across Settings. While you
search, the section list keeps only the sections with matches and shows how many each has. Click a result to jump to it,
or use the arrow keys and Enter from the search field. Escape clears the search before it closes Settings. Cmd+F on a
Mac, or Ctrl+F on Linux, gets you to the field from anywhere in Settings, except that in a Linux text field Ctrl+F moves
the cursor instead.

This page is a map: each section below says what it holds and links the page that explains it.

## Sessions

**Color terminal frames by host** colors each terminal's frame to match its host's icon. **Name sessions on quick
launch** offers to name a session while its agent starts
([Launch templates](/docs/using/launch-templates/#quick-launch)). **Skip session confirmations** makes stop, delete,
replace, restart and closing a shell tab act on the first click
([Stop, restart, and resume](/docs/using/stop-restart-resume/#stop-or-delete)); it does not change the host questions
below, the YOLO question, or the trash's questions.

## Appearance

Choose **System**, **Dark** or **Light**. **System** follows your computer's own setting. The terminal stays dark either
way.

## Hosts

**Update remote hosts automatically** starts on and updates your remote hosts while Farhelm is open
([Update a host](/docs/using/manage-hosts/#update-a-host)). **Set up new hosts without asking** and **Remove hosts
without asking** skip those two questions for SSH hosts when you add or remove one yourself. Turn them off to be asked
again. Taking over a host that another Farhelm manages always asks.
[Agents' host requests](/docs/using/approve-agent-requests/#host-changes) still need your approval.

## Recipes

The shared catalog of recipes that [Tensorlake sandboxes](/docs/using/tensorlake-sandboxes/) and
[container sessions](/docs/agents/containerized-sessions/) start from, with each recipe's Tensorlake snapshot and its
build status on each container host.

## Tensorlake

Your Tensorlake account, the SSH key Farhelm uses for sandboxes, **Create Tensorlake sandboxes without asking**, and
[**Sandboxes not on this helm**](/docs/using/tensorlake-sandboxes/#sandboxes-not-on-this-helm). Deleting a sandbox
always asks.

## Containers

Whether each host is ready to run [Podman containers](/docs/agents/containerized-sessions/), and container leftovers.

## Credentials

GitHub tokens, agent logins and API endpoint tokens that recipes can use
([Save named credentials](/docs/using/tensorlake-sandboxes/#save-named-credentials)).

## Terminal

The [terminal link choices](/docs/using/session-list/#terminal-links), the file types that open with Cmd-click
(Ctrl-click on Linux), and on a Mac, [tunnels](/docs/using/work-in-a-session/#open-a-remote-localhost-link) for remote
localhost links.

## Checkouts

**Default checkout folder**, **Post-clone command** and
[**Keep a checkout ready for faster launches**](/docs/using/start-a-session/#prepared-checkouts), for
[managed checkouts](/docs/using/start-a-session/#start-from-a-managed-checkout).

## Agent trust

The switches that answer an agent's directory-trust question and control the hints shown above the terminal. For
Antigravity the switch covers any directory. For the other agents it covers managed checkouts. See
[when trust settings apply](/docs/using/start-a-session/#trust-in-managed-checkouts).

## Notifications

[Sounds](/docs/using/session-list/#sounds-when-an-agent-needs-you) and
[ntfy alerts](/docs/using/session-list/#ntfy-alerts).

## App

**App** appears only in the installed Farhelm app on your Mac, the one that updates itself. Its **Version** row shows
the release you're running, and **Check for updates** beside it
[checks for an update now](/docs/using/update-and-uninstall/#check-for-an-update-now); the answer appears beside the
version number at the top of the sidebar.

**Install updates automatically** is about updating Farhelm itself
([Turn automatic updates off](/docs/using/update-and-uninstall/#turn-automatic-updates-off)).

## Next

[Approve what agents do](/docs/using/approve-agent-requests/) covers the questions Farhelm asks before an agent acts.
