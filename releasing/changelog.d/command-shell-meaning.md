---
kind: breaking
---

Custom commands, resume commands and agent wrapper prefixes now have full Bash meaning, on hosts and in containers;
wrapped container launches need `bash` in the image. Saved wrappers may need to be checked and re-entered, including in
templates already approved for agents, remembered setups and existing wrapped sessions on their next start or Restart.

`~`, `$VARIABLE`, `*`, `;`, `>`, `&`, `|` and backslashes have shell meaning instead of being passed literally. Saved
wrapper prefixes lost their quotes and escapes when originally saved: `--profile 'work space'` was stored without the
quotes and now passes `work` and `space` as two words. Saved words containing `$`, `*` or quotes now have Bash meaning,
and a word starting with `#` comments out the rest of the agent command. New prefixes retain your quotes and escapes.
Farhelm placeholders must be separate, unquoted words; quoted placeholders are now refused. Filled placeholder values
and Farhelm's composed agent arguments stay safely quoted as literal data.
