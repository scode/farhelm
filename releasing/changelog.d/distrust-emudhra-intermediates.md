---
kind: fixed
---

On Linux, the helm now refuses HTTPS certificates issued by the eMudhra intermediate CAs whose private keys leaked in
October 2026, even on a machine whose CA certificates package has not been updated yet. That covers the downloads made
when adding or updating a host, and sending feedback. On macOS, the operating system already blocks them.

Context for curation: Farhelm trusts the operating system's certificate store and does not check revocation, so on Linux
it trusted these intermediates for as long as the machine's store did. The fix is the rustls-webpki 0.103.16 update,
which rejects any chain containing the leaked keys
([rustls/webpki#546](https://github.com/rustls/webpki/pull/546)). The incident write-up is at
https://emudhra.we-have-a-problem.com/. Release downloads are also protected by their signed checksums, so the exposure
was mainly feedback being readable or blockable by someone able to intercept the helm's traffic.
