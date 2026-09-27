---
kind: changed
---

The web UI is now served only at `http://127.0.0.1:<port>`. Opening `http://localhost:<port>` or `http://[::1]:<port>` redirects to that address, and API or terminal connections under those names are refused. Farhelm listens only on `127.0.0.1`, so on a machine shared with other users, someone else could listen on `[::1]` at the same port, and a browser that tried `localhost` there first would load their page with access to your stored sign-in. If you have been opening the UI at `localhost` on a machine with other local users, rotate the token (`farhelm helm token rotate`) so the sign-in stored under that name stops working, and use `http://127.0.0.1:<port>` from now on.
