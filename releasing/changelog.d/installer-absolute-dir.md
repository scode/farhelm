---
kind: changed
---

The install script now refuses a `FARHELM_INSTALL_DIR` that is not an absolute path. A quoted `~/bin` used to create a
directory literally named `~` wherever the script ran (a later `rm -rf ~` meant to tidy it would delete your home
directory), and any relative path installed under the current directory. Use `$HOME` in the path instead, for example
`FARHELM_INSTALL_DIR="$HOME/bin"`.
