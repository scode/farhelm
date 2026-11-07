---
kind: fixed
---

New terminals in the web UI and desktop app now open straight in the new terminal view, without waiting for the old
terminal library to load or setting up an old-style terminal first. After a Restart moves an old session to a new
terminal, its terminal reconnects once the restart has finished instead of trying while it runs. Old terminals are
unchanged.

Context for curation: until now every terminal was first built with the old terminal library (xterm.js), which also
decided its size, and a new terminal swapped it out when the connection said so. Now the page knows from the session
list which kind each terminal is. The library is still loaded for old terminals. Another window's Restart of an old
session can still take the old route once, until this window's session list catches up.
