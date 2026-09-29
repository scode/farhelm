---
kind: fixed
---

Restarting a session now finishes stopping the old run even if the helm's connection to the host drops part-way.
Before, the stop could be cut off after SIGTERM, leaving the old agent's processes signalled but not confirmed gone and
the session free for another stop or restart while signals were still in flight.
