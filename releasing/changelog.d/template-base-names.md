---
kind: fixed
---

Quick launch can reuse a template's session name for a managed checkout. If that checkout name is taken, Farhelm uses
the next free numbered name, such as `app-review-2`, instead of leaving New unable to launch.

Existing template names become base names without a stored-data rewrite. Typed names and fixed template names still
report a conflict; ordinary folder destinations do not number names.
