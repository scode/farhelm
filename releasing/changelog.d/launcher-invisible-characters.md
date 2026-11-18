---
kind: fixed
---

Editing a copied folder or name in Clone now preserves its invisible characters. The same applies to editable command
fields and Custom flags in the session launcher.

In those fields, editing a value that contains literal text such as `<U+200B>` turns that spelling into the corresponding
character, which still looks the same in the field.
