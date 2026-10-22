---
kind: fixed
---

Bare filenames such as `backup.7z` and `backup.001` in terminal output now offer file hover and download, without needing
a directory prefix.

Context for curation: the version-number check this change widened could pause the app on terminal lines with a very
long run of hyphens after a version-like number, such as `1.1---…---@`. A later unreleased change (#2575) removed that
check's slowdown before any release, keeping exactly which words count as version numbers. Such a line can still take
noticeably long because the file-name check that runs next has its own near-miss cost, which #2575 left alone; whether
that remaining cost also exists in the released version was not established at landing.
