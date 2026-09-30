---
kind: changed
---

The instructions Farhelm gives agents (`farhelm agent instructions`) now tell them never to pass
`--allow-yolo-on-sensitive-host` on their own: an agent whose YOLO launch is refused on a sensitive host reports it to
you and retries with the flag only after you explicitly approve that launch.
