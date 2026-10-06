---
title: Send feedback
description: Tell Farhelm's maintainer something, privately, from inside the app.
sidebar:
  order: 8
---

To tell Farhelm's maintainer something, whether a bug, a wish, or a one-line remark, select the **?** button at the top
of the sidebar, to the right of the gear, and choose **send feedback**. Type your message and, if you want an answer,
how to reach you. Then select **send**.

Your feedback goes privately to the maintainer. It is not a public issue, and you need no account for it. If you would
rather discuss something in the open, you can still [open an issue on GitHub](https://github.com/scode/farhelm/issues).

Nothing is sent until you select **send**. Below your message, the dialog shows everything else that goes with it, and
nothing else does:

- the Farhelm version shown at the top of the sidebar;
- whether you are using the desktop app or the web UI;
- your operating system, by name (macOS, Linux, and so on).

No logs, session contents, host names, or paths are included. Your helm, the part of Farhelm that shows you every host's
sessions ([The pieces](/docs/how-it-works/the-pieces/)), sends the feedback over HTTPS to a feedback service the project
runs. That service sees your helm machine's IP address, which it uses only to limit how often one address can send, and
files your feedback where only the maintainer can read it. Farhelm gives the agents in your sessions no way to send
feedback.

When you enter how to reach you, **Re-use for future feedback** starts checked. After a successful send, your helm
remembers that contact for the next feedback dialog, in both the desktop app and the web UI. Uncheck it, or empty a
prefilled contact field, to forget the contact after sending. If the desktop app or another browser tab is already open,
it shows the new remembered contact after you reload it.

If sending fails, the dialog says so and keeps your text, so you can try again or copy it. The remembered contact stays
as it was.

The same menu's **documentation** item opens these docs in your browser, and in the Farhelm app on your Mac its **check
for updates** item looks for a new release and installs it
([Update or uninstall Farhelm](/docs/using/update-and-uninstall/)).
