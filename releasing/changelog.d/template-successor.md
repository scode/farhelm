---
kind: fixed
---

An agent's session created from a template that names its host could, in a narrow window, start on a different
machine than the one the template was made for: when that host entry had just been pointed at another machine and the
new installation adopted while the request was being handled. Such a request is now refused, saying the host now
reaches a different installation, and nothing is started. A request that names its host with `--host` is unaffected.
