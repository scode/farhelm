# A YOLO preference reply can inspect a different confirmation

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A permanent permission reply checks whichever launcher question is in flight. Whether a later question can receive the
older launch permission remains unverified.

## Details

**other.** Investigated uncertainty.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:120–146;
crates/gui/farhelm-gui/src/launcher/view/recents.rs:105–125;
crates/gui/farhelm-gui/src/launcher/view.rs:959–974,1297–1316;
crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:213–223,236–253,337–389;
f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6:crates/gui/farhelm-gui/src/launcher/view/recents.rs:183–202,294–309

The ordinary recent-submit path makes a launch without one-time permission and edits retire the previous inline
question. The permanent preference reply checks whether the launcher currently has any YOLO question in flight. When
that check succeeds, it uses the live draft and the previous answer's consent. It carries no question identity. Host
writes use per-host operation keys. The ordering needed to have a later eligible question in flight before the earlier
callback settles has not been established in the shipped UI channel. See
`launcher/view/yolo_question.rs:236–253,364–389`, `launcher/view.rs:959–974` and `operations.rs:842–894`. Bind the reply
to the question and draft it was issued for.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/recent-submit-during-yolo-setting-write`.
