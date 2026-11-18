---
kind: fixed
---

Settings → Containers no longer lists a container that belongs to one of your hosts as a container not on this helm
when you have two host entries that reach the same machine and account (a LAN name and a tailnet name for one box,
say). Before, deleting it from that list destroyed the live container with its sessions and files.
