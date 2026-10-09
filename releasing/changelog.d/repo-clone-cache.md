---
kind: changed
---

Repeated fresh GitHub checkouts are faster because each host keeps a repository cache and downloads only new objects before cloning. Every checkout stays independent of the cache. Caches take roughly the space of one extra copy of each repository and are removed when the supervisor starts after more than 30 days without use. A private repository without a credential helper can ask for credentials twice; a cache failure stops the checkout and names the cache path.
