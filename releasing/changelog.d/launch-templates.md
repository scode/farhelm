---
kind: added
---

Launch templates: named sets of launcher edits, which you create, edit and delete in the Templates panel beside New. A template can set any launcher field (the kind of launch, the agent and its model, effort, permissions and workspace trust, a command and its YOLO answer and resume command, the host, the folder or a fresh GitHub checkout, and the session name), and every field is optional. In the session launcher, type `tl:` and a template's name in the search box to apply it: it makes exactly the edits it contains, as if you had made them by hand, and you can apply several in turn, the later one winning where they overlap. A template with a field that does not fit the launcher as it is (a model while you are on the command tab, a host in Replace with) is refused with the field named, and nothing from it is applied. Sessions do not record which templates made them, so editing or deleting a template never affects a session.
