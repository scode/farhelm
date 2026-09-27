---
kind: fixed
---

The helm now accepts browser requests only from its own `http://` address. Before, a helm started with `--port 80` also accepted requests from a page served over HTTPS on `https://127.0.0.1` (port 443, a different local server) as if they came from the helm's own page. Such requests still needed a device secret.
