---
kind: fixed
---

Emoji such as ❤️ now take two columns in new terminals, keeping the text after them aligned. Emoji and text
presentation selectors follow Ghostty's defaults, with two remaining edge differences described on the new-terminals
documentation page.

The exceptions concern a printed space at the right edge with wrapping disabled, and ordinary wide characters in a
terminal only one column wide. Old terminals still use tmux's rules.
