---
kind: fixed
---

The Mac app and Safari gain a partial workaround for [WebKit's input-method event order](https://bugs.webkit.org/show_bug.cgi?id=165004):
after earlier typing keys have been released, Enter or Escape immediately ending composition leaves the dialog and
draft intact. Holding an earlier typing or modifier key, or a delayed ending event, can still submit or close the dialog.

Browser tests replay both event orders. Real input-method verification in the Mac app remains a maintainer check. The
workaround favors leaving the next ordinary key usable when overlapping keys are ambiguous; its compatibility hint
expires after 16ms, even for key code 229.
