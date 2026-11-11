---
title: Security model
description: What Farhelm exposes and trusts, and the connections it makes on your behalf.
sidebar:
  order: 3
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

What Farhelm exposes and trusts: SSH for reaching hosts, a web UI that listens only on loopback and that a link on a
page from another local port cannot open, and no Farhelm relay or account. Adding an ordinary SSH host needs no root;
[Tensorlake sandboxes](/docs/using/tensorlake-sandboxes/) use passwordless sudo to prepare user services and need a
Tensorlake account. Also how much the helm is trusted with on each host, since it can run commands there, and what an
agent on one host can do to sessions on others through Farhelm, with your approval or a host's **Run farhelm commands
without asking**. The narrower **Manage sessions on this host without asking** covers session actions only on that same
host, including a copy's source; it grants no authority over other hosts or templates. Both choices still need the helm,
the program that shows your hosts' sessions and serves the UI, to be reachable
([The pieces](/docs/how-it-works/the-pieces/)). Host changes (adding, editing or removing a host) always need your
approval ([Approve what agents do](/docs/using/approve-agent-requests/)). The SSH prerequisite for adding a host is in
[Add a remote host](/docs/get-started/add-a-remote-host/). It will also cover the connections that leave your machines:
setting up or updating a host, including the automatic updates you can turn on, downloads Farhelm's release files from
get.farhelm.io, which hands them over from GitHub, feedback you choose to send goes over HTTPS to a feedback service the
project runs ([Send feedback](/docs/using/send-feedback/)), and the Farhelm app on your Mac checks get.farhelm.io for
new releases and downloads them, checking each one's signature, by itself unless you turn that off
([Update or uninstall Farhelm](/docs/using/update-and-uninstall/)). If you set up
[ntfy alerts](/docs/using/session-list/#ntfy-alerts), your helm sends a waiting session's name and its host's name to
the ntfy server you chose, where anyone who knows the topic can read them. And downloading a file from a session's
terminal: the helm can read any file the session's account can read on that host. In the desktop app, Cmd-click
(Ctrl-click on Linux) also opens the saved copy in its default app for `.md`, `.txt` and `.json` files by default. The
[desktop setting](/docs/using/session-list/#terminal-links) can allow any file type you add, including one that may run
code in its default app. The dialog warns beside each type Farhelm does not recognize as safe. An opened HTML-like page
can run browser scripts and send requests to services on your computer, including localhost on other ports. Plain clicks
and browser clicks only download; check the disclosed host and path first
([Work in a session](/docs/using/work-in-a-session/#copy-paste-and-links)). And opening links: with your approval, or a
host's link permission, an agent on that host can make your browser open a web address, and the local-network permission
lets it reach addresses on your own machine and network without asking. The Mac app's optional localhost tunnels also
let you open a remote development server as a local page, after accepting a separate warning about cookies,
local-network access and other programs using the tunnel
([Open a remote localhost link](/docs/using/work-in-a-session/#open-a-remote-localhost-link)). It will also cover
[container session containment](/docs/agents/containerized-sessions/#proxies-and-containment). The checkout and selected
credentials live inside, with the host's home and checkout kept outside. Rootless Podman supplies containment. Farhelm
leaves network access, CPU and memory use unrestricted.

It will also cover Tensorlake's API, which the helm calls over HTTPS with your saved API key, Farhelm's own SSH key for
those sandboxes, and the API key and agent endpoint tokens stored unencrypted on the helm's machine. A recipe's
[tunnels to your side](/docs/using/tensorlake-sandboxes/#tunnels-to-your-side) let anything in the sandbox or container
reach the configured services through the helm's machine. These tunnels use the host's existing connection, including
your SSH configuration and **ProxyJump**.
