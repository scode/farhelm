---
title: The pieces
description: "The Farhelm app, the helm, supervisors and hosts: what each one is and where it runs."
sidebar:
  order: 1
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

The mental model in one diagram and a paragraph per piece: the Farhelm app, on a Mac or on Linux (the installer covers
Macs only so far), whose window is what you look at and which runs the helm (the one control plane) inside it; a
supervisor on each host (the owner of that host's sessions); and the hosts themselves, the computer the app runs on and
the machines you add. Also where each one runs, and that the supervisor on your Mac keeps running after you quit
Farhelm.
