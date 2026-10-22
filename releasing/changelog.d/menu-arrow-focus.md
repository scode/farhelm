---
kind: fixed
---

Menu arrow keys no longer pull focus back after you move to another control while a focus move is still on its way to
the desktop window.

Context for curation: the keyboard focus tags this fix added to menu items briefly carried an internal memory address of
the desktop app into the page. A later unreleased change (#2569) made the tags a plain counter before any release, so
no shipped version exposes the address and the entry above needs no caveat.
