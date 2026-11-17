//! Tests of the command well: Copy writes the command's raw bytes and then tells its owner. Its look, the wrap of a
//! long command included, is pinned by the `command-well` golden.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, TestAppContext};

use crate::components::{CommandWell, HoverHelp};
use crate::host_text::HostText;
use crate::test_support::{click, label, open};

/// A pasted command must be the helm's exact bytes while the screen escapes what a host could hide (SPEC_impl.md,
/// "Host-supplied text"), and Tensorlake's "Command copied" must follow a copy the user can already paste. Specified:
/// a command holding a BEL is named with `<U+0007>` under the well's id; with the clipboard empty, a click on
/// `{id}-copy` puts the raw command on it, and `on_copy` runs once, with the command already there.
#[gpui::test]
fn copy_writes_the_raw_command_and_then_tells_the_owner(cx: &mut TestAppContext) {
    let raw = "ssh -- 'dev@build\u{7}box'";
    let seen: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let record = seen.clone();
    let cx = open(cx, move |_, _| {
        let record = record.clone();
        let command = HostText::from_host(raw.to_owned());
        CommandWell::new(
            "cmd",
            command,
            "Copy",
            HoverHelp::tooltip("Copy the command"),
        )
        .on_copy(move |cx| {
            let held = cx.read_from_clipboard().and_then(|item| item.text());
            record.borrow_mut().push(held);
        })
        .into_any_element()
    });
    assert_eq!(
        label(cx, "cmd").as_deref(),
        Some("ssh -- 'dev@build<U+0007>box'")
    );
    assert_eq!(
        cx.read_from_clipboard(),
        None,
        "the premise: nothing copied yet"
    );
    click(cx, "cmd-copy");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(raw));
    assert_eq!(
        *seen.borrow(),
        [copied],
        "on_copy ran once, after the write"
    );
}
