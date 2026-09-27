---
kind: fixed
---

A remote host can no longer sign your browser out of the web UI. When a host refused an action on one of its sessions with an authorization error, the helm passed that on as its own "sign in again" answer, so a misbehaving or compromised host could replace the whole UI, every host's sessions included, with the token prompt each time you touched its sessions. Such refusals now show as an ordinary error on that action. A host also can no longer make the session launcher discard a pending launch and reload by wording its error message a particular way.

After updating, reload any browser tab that was open during the update so it picks up the new launcher behavior; an old tab shows a stale-connection refusal as a plain error instead of reloading the host.
