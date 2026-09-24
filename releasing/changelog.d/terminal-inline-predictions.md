---
kind: fixed
---
On macOS, typing in a terminal could insert words you never typed, such as `SPECIALLY` after typing `SPE` in Codex, apparently because macOS inline predictive text completed words in the terminal's input. Farhelm now turns inline predictions off for terminal input. The corruption has never been reproduced reliably, so this fix is unconfirmed, and it may not cover every form of the corruption that has been reported.
