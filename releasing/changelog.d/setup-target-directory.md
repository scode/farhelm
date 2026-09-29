---
kind: fixed
---

`farhelm helm setup` no longer refuses an installed `farhelm` whose path merely contains a directory called `target`,
such as `/opt/target/bin` or the home directory of a user named `target`. It still refuses a binary run straight out of
a Cargo build directory (`target/debug` or `target/release`).
