"""Remove files the previous sync sent that the working copy no longer tracks.

Usage: prune.py <base>, with the new list of tracked paths (relative to the checkout) on stdin. `<base>/synced.list`
holds the previous sync's list; every path in it that the new list lacks is deleted from `<base>/src`, then the new list
replaces it. Only paths a sync put there are ever deleted, so the worker's build output and the test suites' own files
under `src/` (all ignored by git) stay. A path that tries to leave `src/` is refused.
"""

import os
import sys

base = sys.argv[1]
src = os.path.realpath(os.path.join(base, "src"))
known = os.path.join(base, "synced.list")
new = [line.rstrip("\n") for line in sys.stdin if line.strip()]
old = []
if os.path.exists(known):
    with open(known, encoding="utf-8") as f:
        old = [line.rstrip("\n") for line in f if line.strip()]
keep = set(new)
removed = 0
for rel in old:
    if rel in keep:
        continue
    path = os.path.realpath(os.path.join(src, rel))
    if not path.startswith(src + os.sep):
        sys.exit(f"prune.py: refusing {rel!r}, outside the tree")
    try:
        os.remove(path)
        removed += 1
    except FileNotFoundError:
        pass
with open(known + ".tmp", "w", encoding="utf-8") as f:
    f.write("\n".join(new) + "\n")
os.replace(known + ".tmp", known)
if removed:
    print(f"pruned {removed} file(s) the working copy no longer tracks")
