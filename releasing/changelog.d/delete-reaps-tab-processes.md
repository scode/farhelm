---
kind: fixed
---

On a Mac without systemd, deleting a session could leave programs started in its terminal tabs running (a backgrounded
`ssh -N`, `caffeinate`, a `nohup`'d command) while reporting the session deleted, with nothing left in Farhelm to stop
them. Delete now stops a tab's processes the way closing that tab does.
