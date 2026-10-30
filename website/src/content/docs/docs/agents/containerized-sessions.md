---
title: Containerized sessions
description: Run an agent and its terminal tabs in a rootless Podman container on a Linux host.
sidebar:
  order: 61
  badge:
    text: Experimental
    variant: caution
---

:::caution[Experimental]

Containerized sessions are Linux only and may change quickly. You supply the image and configure the agent in it.
Farhelm does not build or download images, and subscription logins are not supported.

:::

A containerized session runs its agent and every [terminal tab](/docs/using/work-in-a-session/) inside one rootless
Podman container. The container has access to the session's managed checkout, one local API proxy port, and an optional
GitHub token. Other host directories and Farhelm's own credentials are kept outside it.

:::caution[Git hooks in the checkout]

The agent can change anything in the checkout, including the Git hooks and settings in its `.git` folder and an agent's
own project settings. Those can run with your account's full access when Git or an agent later runs in that checkout
outside the container: from your terminal, a shell prompt that shows Git status, your editor or a later ordinary
session. Treat the checkout as untrusted after a containerized session and check its `.git` folder and agent settings
before entering it from the host.

:::

The Linux host running the session needs rootless Podman with `pasta` networking configured for your account. The API
proxy or load balancer runs on that same host, alongside its [supervisor](/docs/how-it-works/the-pieces/), and listens
on `127.0.0.1` at the port you choose. The desktop app can launch one of these sessions on a Linux host; its own Mac
cannot run the container.

## Prepare an image

Load an image with `podman load` or build one with `podman build` on the Linux host where the session will run. It must
already be in that account's local Podman storage. This feature has been tested with Ubuntu 24.04 and Podman 4.9.3.
Farhelm never imports, pulls, tags or removes images.

The image needs:

- The agent programs and any [wrapper](/docs/agents/agent-wrappers/) you use available to the account Farhelm chooses
  below, with their configuration ready for that account. Configure the agent to use your API proxy at
  `127.0.0.1:<port>` inside the container.
- Ubuntu's normal account tools (`useradd` and `groupadd`) and `/etc/skel` for creating a home when the image does not
  already have a suitable account.
- `bash` for terminal tabs and `sleep` for keeping the container running between commands.
- `git` and, if you want GitHub access, `gh`. Configure Git to use `GH_TOKEN`, for example with `gh auth setup-git` in
  the image.

Farhelm tries to match your host account's name, numeric user and group IDs, and home path. It checks the image before
mounting the checkout and chooses the account in this order:

1. If neither your user name nor your user ID exists in the image, Farhelm creates your account there and seeds its home
   from `/etc/skel`.
2. Otherwise, if your host home path already exists in the image as a directory owned by a non-root account, use that
   account with that directory as its home.
3. Otherwise, if your user ID exists in the image, use that account and its home.

For example, a host account with user ID 1000 and a stock Ubuntu image still uses `ubuntu` and `/home/ubuntu` if the
host home path is absent from the image. A different host user ID gets a new account when neither its name nor ID
exists. An image prepared only for its user-1000 account may then lack the agent, its configuration or `sudo` access for
the new account. Install agents system-wide and put default home configuration in `/etc/skel`, or prepare your own
account and home in the image. Farhelm maps whichever account it chooses to your host account, so checkout files it
writes still belong to you. Existing containers keep their account until you replace them.

You may include `sudo` and configure access for the account the session uses; Podman's default privileges inside the
container are retained. Keep host credentials out of the image unless you intend every agent using it to have them.

Farhelm does not run the image's usual startup command (`ENTRYPOINT` or `CMD`). Agent configuration and executable paths
must already be ready in the image, rather than set up by that command.

## Start a session

In the [session launcher](/docs/using/start-a-session/), choose a managed checkout and turn on **Containerized** on the
**Agent** tab. Supply **Image**, its local name or ID (for example `localhost/agent:latest`), and **API proxy port**, a
number from 1 to 65535. **GitHub token file** is optional: an absolute path to a host file containing the token. Leave
it blank to pass no token.

The host file is read afresh when the agent starts and whenever you open a terminal tab. Surrounding whitespace is
trimmed. Farhelm passes its contents as `GH_TOKEN`, without saving the token in the session or putting it on a command
line. Your own Podman tools can see that value while the command receiving it is running.

The summary shows `container`. New sessions start with the switch off and the inputs empty. Recent setups, Clone,
Replace with and [launch templates](/docs/using/launch-templates/) carry the settings when you choose them. Existing
directory launches outside a managed checkout and the **Command** tab do not offer containerization.

Farhelm refuses an image that is missing from that host's local Podman storage. Rebuilding an image under the same name
affects the next new container, including Replace and Replace with; an existing container keeps its original image.
Container startup diagnostics appear in the session's terminal.

Settings saved by earlier versions that used an archive path now have an empty **Image** field. Fill it in before
launching, edit the template, or use **Replace with** to choose an image for an old session.

## Working inside the container

The managed checkout is the only host directory Farhelm mounts, read-write, at the same absolute path on both sides. The
agent and terminal tabs work there, using the same container account and `HOME`. Your host home directory is not
mounted; programs and configuration elsewhere come from the image. A terminal file link is offered only when its
destination stays inside the checkout, including after following symlinks.

Printed paths and file hyperlinks supplied by terminal programs follow the same checkout boundary, including filenames
containing spaces in an explicit hyperlink. Web hyperlinks remain available.

The selected proxy port connects container `127.0.0.1:<port>` to host `127.0.0.1:<port>`. Other host loopback ports are
not forwarded. Outbound internet access stays open, and the host's LAN, tailnet and cloud metadata addresses remain
reachable. This is not a network-isolation feature. Your account's `containers.conf` still applies, including any extra
mounts or environment settings you put there.

Farhelm imposes no CPU or memory limits on the container; its programs can still exhaust the host's resources.

Farhelm's conversation integration is off inside the container. It still reads the agent's screen for status, but it
does not follow the conversation or warn about missing conversation reports, and `$farhelm` requests are unavailable.
The host's Claude trust configuration is not changed. **Restart** and **Restart with** are unavailable; **Replace** or
**Replace with** starts a fresh session and container.

## Stopping and deleting

[Stop](/docs/using/stop-restart-resume/) stops the whole container, including its terminal tabs and processes that
detached from a tab. Closing just one tab ends its shell; a process it detached may keep running until the container
stops. The container also stays running after the agent exits on its own. Opening a tab after Stop starts the existing
container again.

Delete removes the session's container, including detached work still running after the agent and its tabs have ended.
Its confirmation follows the agent and tabs shown in Farhelm; it does not inspect those detached processes. Images in
Podman's storage remain until you remove them yourself. After a host reboot, the session's terminal is gone and the
session can only be replaced or deleted.

Programs running as root inside the container create checkout files owned by a subordinate host UID. You may be unable
to edit or remove them normally. Use `podman unshare` on the host to fix or remove them. The same issue can prevent
emptying the Trash when a checkout contains a directory created by container root; `podman unshare rm -rf` can remove
that directory. Check its path carefully: this deletes its contents permanently.

Farhelm's localhost tunnels still reach the session host, including tunnels offered for a URL the agent prints. They do
not reach a server running inside the container. See
[localhost tunnels](/docs/using/work-in-a-session/#open-a-remote-localhost-link) for their ordinary behavior.
