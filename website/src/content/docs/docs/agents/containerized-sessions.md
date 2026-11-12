---
title: Container sessions
description: Give a session its own rootless Podman container, with its checkout and Farhelm running inside.
sidebar:
  order: 61
  badge:
    text: Experimental
    variant: caution
---

:::caution[Experimental]

Container sessions need a Linux host with rootless Podman. They may change quickly. **Deleting the last session in a
container permanently removes the whole container and its files, including unpushed work.** Suspend keeps its memory in
use on the host.

:::

A container session gets a new container on one of your hosts, built from a saved recipe. Its agent, terminal tabs and
checkout live inside. Farhelm's [supervisor](/docs/how-it-works/the-pieces/), the program that keeps sessions running,
runs there too, so the session has ordinary hooks, conversation tracking, Restart, file transfers and localhost tunnels.

For the cloud equivalent, see
[Tensorlake sandboxes](/docs/using/tensorlake-sandboxes/#give-each-session-its-own-sandbox).

## Choose a container host

Add the Linux machine as an ordinary [SSH host](/docs/get-started/add-a-remote-host/), or use your local Linux host. It
needs Podman **4.9.3 or later**, running rootless with **cgroup v2**, the **systemd cgroup manager**, and a working
`systemd-run --user`. Farhelm offers every usable host automatically. You can launch from a Mac to a Linux container
host. Docker, macOS container hosting and nested environments are unsupported.

Opening the launcher checks hosts in the background. **Settings → Containers** shows each host's readiness and the
reason it cannot be used. **Refresh** checks again. A selected host that becomes unusable stays selected, with its
reason visible.

On an SSH host, container connections share the host's SSH connection with its ordinary supervisor. The SSH server's
usual ten-channel limit means **about ten concurrent container connections** when each new SSH login needs interactive
approval. Other channels and server settings change the actual capacity. OpenSSH can fall back to another noninteractive
login, and a successful fallback can allow more connections. If that fallback fails after the shared connection fills,
Farhelm reports the container host's SSH session limit.

## Choose a recipe

Open **Settings → Recipes** to create or edit a recipe. The catalog is shared with Tensorlake.

- The managed Ubuntu base works for both containers and sandboxes.
- A Tensorlake image or snapshot works only for Tensorlake.
- A container image reference with its login user works only for Podman.

A custom base must boot systemd, allow the login user to use sudo without a password, be Ubuntu or Debian with apt, and
include bash, curl and git. Farhelm checks those requirements and adds the packages you choose, Claude Code and Codex.
It may pull a missing base image on the container host. CPU, memory and disk size choices apply only to Tensorlake.

Recipes also choose
[API endpoints, subscription logins and tunnels](/docs/using/tensorlake-sandboxes/#agent-api-endpoints), and a
[GitHub token for private checkouts](/docs/using/tensorlake-sandboxes/#managed-checkouts). Save named credentials in
**Settings → Credentials** first. These settings work for containers as well as sandboxes. The recipe supplies the
container's login user, endpoints and credentials.

The Recipes list shows each container host's build status with **Rebuild**, beside the Tensorlake snapshot's own **Build
snapshot** or **Rebuild snapshot**. A container's first launch on a host builds any missing software image. On later
launches, Farhelm prepares it again when more than 24 hours old, after a Farhelm update or after the software image is
rebuilt. Existing containers keep their software. Rebuild affects future launches. User-owned base images are retained.

**Settings → Tensorlake** keeps the cloud account's API key, SSH key, creation confirmation and sandbox leftovers.
**Settings → Containers** keeps host readiness and container leftovers. Recipes are edited in **Recipes** for both.

## Start a session

Open **+** (New session). In [the session launcher](/docs/using/start-a-session/), **Runs on** offers **Existing host**,
**New container** and **New sandbox** when those choices are available. Choose **New container**, then **Recipe** and
**Container host**. The host defaults to the one you used last. Recipes that cannot run there show why.

Choose a **Managed checkout** to clone a GitHub repository inside the new container. **Folder** is fixed to the
container user's home directory. Directory browsing and recent host directories are unavailable for this destination.
The checkout and home live inside the container, with the host's checkout and home kept outside. The **Agent** and
**Command** tabs both work.

Launch progress shows preparation when needed, container creation, starting Farhelm inside it, then the ordinary session
launch. The task continues after you close your window. A failed initial launch attempts to remove its new environment.
Failed cleanup or an interrupted launch remains available for explicit deletion in **Hosts** or **Containers not on this
helm**. An interrupted launch stays interrupted when Farhelm starts.

[Templates and quick launch](/docs/using/launch-templates/) save the recipe and container host together. Use the
template editor to save that choice. **Save as template** is unavailable for an environment launch. Recent setups
restore the same choices even after you delete the old container. **Clone** opens the launcher for a fresh container
from the recipe with the same repository choice and a fresh disk. Opening **+** (New session) while a container session
is selected also starts from its recipe and host. A removed host or deleted recipe stays unavailable until you change
the choice.

## Working inside the container

[Terminal tabs, attachments, file links and downloads](/docs/using/work-in-a-session/) work inside the container as on
an ordinary host. File links can reach any file its account can read there. A localhost tunnel reaches the container's
loopback, including when the container host is the same machine as the helm, the program that shows all your sessions
([The pieces](/docs/how-it-works/the-pieces/)). Recipe tunnels carry traffic in the other direction, to services
reachable from the helm's machine.

[Restart and Replace](/docs/using/stop-restart-resume/) stay in this container. Restart resumes the agent's conversation
where the agent supports it. Replace starts a new session before removing its source session. It preserves the container
and files if replacement fails or the outcome is uncertain.

Containers appear in **Hosts**, the session list and the usual update and problem notices. The permanent icons in the
[navigation rail](/docs/using/manage-hosts/), the column of icons at the sidebar's left edge, are reserved for ordinary
hosts. A container session you start yourself always gets a fresh **New container** launch; an agent inside a container
can start more sessions in that same container.

An agent in a container **can ask Farhelm to act only within its own container**, as
[Agents in sandboxes and containers](/docs/using/approve-agent-requests/#agents-in-sandboxes-and-containers) explains.
Agents on other hosts cannot delete container sessions, and they start new containers only behind a card that always
asks ([New sandboxes and containers](/docs/using/approve-agent-requests/#new-sandboxes-and-containers)).

### Proxies and containment

At creation, the container inherits proxy variables from its host's **systemd user manager environment**, as shown by
`systemctl --user show-environment`. Set them in that manager to configure these defaults. Farhelm disables Podman's
implicit HTTP-proxy inheritance and puts the manager's values into the container user's `environment.d`, so its services
and sessions get the same settings. Later host changes apply to new containers.

Rootless Podman supplies the containment. Farhelm keeps the host's checkout, home and Farhelm socket outside the
container. Internet, LAN, tailnet and cloud metadata addresses remain reachable. Farhelm leaves network access, CPU and
memory use unrestricted, so programs can exhaust host resources. Your account's Podman configuration still applies,
including extra mounts or environment settings you configure yourself.

## Stopping and deleting

**Stop** ends the chosen agent and its tabs, leaving the container running. **Suspend** pauses the whole container.
Processes freeze and their memory stays allocated on the host. **Resume** continues them. **Stop and suspend** stops the
chosen session first, then pauses the whole container, including any leftover sessions.

A host reboot, external `podman stop` or an out-of-memory kill leaves the container **Stopped**, with files retained.
When Farhelm is connected to the container host, it attempts to start a stopped container once. Failure shows the reason
and offers **Resume**. Sessions whose running agent was lost to a host reboot show **Interrupted**. Sessions that had
already exited keep their status, and stopping a container without rebooting its host leaves its agents exited. Choose
**Restart** to resume a captured conversation. Restarting the helm reconnects running containers and leaves paused ones
paused.

**Delete** always asks first, because the container may go with the session. Its confirmation says both outcomes: the
session and its checkout are deleted, and if it is the last session in the container, the whole container and its files
are deleted too. Unless another session still uses it, the checkout is deleted for good; it does not go to the
[checkout trash](/docs/using/session-list/#managed-checkout-trash). If other sessions are still there, for example ones
an agent started or one a failed Replace left behind, the container stays and Farhelm tells you how many ("The container
stays: 2 other sessions are in it"). An agent in the container that deletes another session removes only that session,
never the container.

Delete checks with the Farhelm running inside the container, the only place that knows every session in it. When that
check cannot happen, because the container is paused or unreachable or its Farhelm needs an update, or it gets no
answer, Delete offers **Delete session and container** instead. Its confirmation warns that the container, every session
and process in it, its checkouts, files and unpushed work will be destroyed, including sessions Farhelm does not list.
That works while paused or while the supervisor is disconnected, without Resume, if the container host is reachable. To
delete just the one session, resume or update the container first.

While Farhelm deletes a container, **Hosts** and the sidebar show it as **Deleting…**. If that fails after its last
session was deleted, it shows **Deleting failed** with the reason, and **Retry** deletes it again. Farhelm forgets the
container only after confirming its removal. Deleting several marked sessions works one session at a time in the same
way, as for [sandboxes](/docs/using/tensorlake-sandboxes/#delete-a-session-in-its-own-sandbox).

**Delete container** in Hosts always destroys the whole container after its own confirmation. A failure there keeps the
container and reason visible so you can retry.

**Containers not on this helm** in Settings → **Containers** lists discoverable leftovers for explicit deletion. Check
the name and host before confirming. You must clean up environments and recipe images recorded on a host before removing
or changing that host's destination. A refusal names what still needs cleanup.

## Earlier containerized sessions

The earlier experimental feature ran only the agent and tabs inside a container and mounted a host checkout. Those
sessions now support **Delete only**. Their rows are marked **Retired container**, and their agents keep running until
you delete them. Terminal access, Stop, Restart, Clone and Replace are unavailable. Stored templates and recent setups
still load. Remove their retired container choice before launching again, then select a recipe-based container in the
launcher or template editor.

Continue with [Start a session](/docs/using/start-a-session/) or
[make a shared recipe](/docs/using/tensorlake-sandboxes/#make-a-recipe).
