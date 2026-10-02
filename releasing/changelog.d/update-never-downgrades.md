---
kind: fixed
---

Update no longer installs an older Farhelm over a newer one. A host whose supervisor runs a newer version than the helm
used to be offered Update (alone and in "update all"), and running it replaced the newer build with the helm's older
one; across a database change that left the host unreachable until the newer build was reinstalled by hand. Such a host
is now labelled "too new" in the host list, where hovering the label shows both versions; it is not offered Update, and
an Update request for it is refused, naming both versions. Update the helm instead.
