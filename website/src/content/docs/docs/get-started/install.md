---
title: Install Farhelm
description: Install the Farhelm desktop app on an Apple silicon Mac.
sidebar:
  order: 1
---

Farhelm runs as a desktop app on your Mac, and for now that is the one way to run it: the installer supports Macs with
Apple silicon only. Linux machines can still run your sessions, as hosts your Mac reaches over ssh; you add those from
inside the app once it is running ([Add a remote host](/docs/get-started/add-a-remote-host/)). Support for running
Farhelm itself on Linux is coming.

## Install

Run this shell command in a terminal:

```sh
curl -fsSL https://get.farhelm.io/install.sh | sh
```

It needs no administrator password. It puts the Farhelm app in `~/Applications` and the `farhelm` command in
`~/.local/bin`, and finishes by telling you Farhelm is installed and how to uninstall it later. After that, Farhelm
keeps itself up to date, installing new releases in the background; see
[Update or uninstall Farhelm](/docs/using/update-and-uninstall/). Running the installer again also updates Farhelm, and
that is safe while Farhelm is open: Farhelm notices the new version, and its version number turns red until you restart
it.

## If the installer asks for tmux

Farhelm runs every session inside tmux, and needs tmux 3.7c or newer on your Mac. The installer may tell you that your
Mac has none, or one that is too old. If it does, install or upgrade it with Homebrew:

```sh
brew install tmux
```

(`brew upgrade tmux` for an older one; if you do not have Homebrew yet, [brew.sh](https://brew.sh/) has the one-line
install.) Farhelm will not start until a new enough tmux is in place. If the installer does not mention tmux, you
already have what you need.

## Open Farhelm

Open Farhelm from Spotlight or from `~/Applications`. The app starts everything it needs by itself: there is no
background service to set up and nothing to run in a terminal. Your Mac is ready to run sessions as soon as the window
opens. [The pieces](/docs/how-it-works/the-pieces/) has what runs where, for when you want the model behind it.

To remove Farhelm again, see [Uninstall Farhelm](/docs/get-started/uninstall/).

Next: [Your first session](/docs/get-started/first-session/).
