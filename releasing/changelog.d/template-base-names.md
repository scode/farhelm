---
kind: fixed
---

Quick launch now numbers a template's session name when its managed-checkout directory is taken, so you can reuse the
setup. Templates and Save as template offer fixed names too, which report a clash instead.

Existing template names become base names without a stored-data rewrite. Typed names and fixed template names still
report a conflict; ordinary folder destinations do not number names.

Agents can choose either kind with `farhelm agent template create` or `edit --title-kind base|fixed`; template listings
show the effective kind.

Agent-created managed checkouts use the same base-name numbering, with the chosen title and folder shown for approval.
An agent's own `--title` stays exact, and keyed retries keep their original checkout.
