---
kind: fixed
---

Updating a host now refuses, before changing anything, when a newer build already ran there and upgraded its saved data
past what the update can read. Before, the update went ahead and left the host unreachable until that newer build was
put back.

This mostly happens when going back to a release after trying a development build on that host.

The protection needs this check in the updating app and database-version reporting in the build that ran on the host.
Development builds made before this reporting was added are not detected, so an update back from one can still leave
the host unreachable.
