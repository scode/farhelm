---
title: Security model
description: Why SSH is the only network path between your machines, and what that means for them.
sidebar:
  order: 3
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

What Farhelm exposes and trusts: SSH as the only network path between machines, a web UI that listens only on loopback,
no relay or account, and nothing that needs root. Also how much the helm is trusted with on each host, since it can run
commands there. The SSH prerequisite for adding a host is in [Add a remote host](/docs/get-started/add-a-remote-host/).
It will also cover the connections that leave your machines: setting up or updating a host downloads Farhelm's release
files, feedback you choose to send goes over HTTPS to a feedback service the project runs
([Send feedback](/docs/using/send-feedback/)), and the Farhelm app on your Mac checks GitHub for new releases and
downloads them, by itself unless you turn that off ([Update or uninstall Farhelm](/docs/using/update-and-uninstall/)).
