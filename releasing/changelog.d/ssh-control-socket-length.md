---
kind: fixed
---

Remote hosts no longer all fail to connect when your username is long (16 or more characters on Linux, 11 or more on
macOS, with the default state directory), or when the helm's state directory is deep. The ssh connection-sharing socket
Farhelm uses was too long for the system limit, and ssh gave up before reaching the host while the hint blamed a missing
supervisor. Where the socket still cannot fit, for example with a macOS username of 18 or more characters, Farhelm now
connects without connection sharing, which works but makes each new connection slower.
