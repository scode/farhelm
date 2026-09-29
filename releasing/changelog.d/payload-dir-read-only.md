---
kind: fixed
---

`farhelm helm run --payload-dir` no longer needs write access to the directory it names. Farhelm used to unpack the
release files into a hidden folder inside it, so a read-only copy, a root-owned directory, or a mirror shared with other
users made every host setup and update fail with a permission error. The unpacked copies now go to the helm's own state
directory. A hidden `.farhelm_extract_tmp` or `.extracted` folder an older version left in your payload directory is no
longer used and can be deleted.
