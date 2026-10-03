---
kind: breaking
---

The installer now runs only on Apple silicon Macs, where it installs the desktop app and the `farhelm` command. Linux remains supported for helms and session hosts, but the installer temporarily refuses Linux. It always uses `~/.local/bin` and builds `~/Applications/Farhelm.app`; `FARHELM_INSTALL_DIR` and `FARHELM_NO_APP_BUNDLE` no longer select another setup. Use `FARHELM_VERSION` as before, with version 0.2.1 or newer: releases without the Mac app resources are refused before replacing installed files.
