// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").

use std::{cell::RefCell, rc::Rc};

use gpui::TestAppContext;

/// An animation driver must see the new preference before the setter returns, including when motion resumes.
/// Reapplying the same platform value should not start or interrupt work, and the guard owns the callback's lifetime.
#[gpui::test]
fn reduce_motion_observer_reports_changes_until_dropped(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update(|cx| {
        cx.set_reduce_motion(false);
        assert!(!cx.reduce_motion());
        let observed = observed.clone();
        cx.observe_reduce_motion(move |cx| observed.borrow_mut().push(cx.reduce_motion()))
    });
    assert!(observed.borrow().is_empty());

    cx.update(|cx| {
        cx.set_reduce_motion(false);
        assert!(observed.borrow().is_empty());
        cx.set_reduce_motion(true);
        assert_eq!(observed.borrow().as_slice(), &[true]);
        cx.set_reduce_motion(true);
        cx.set_reduce_motion(false);
        assert_eq!(observed.borrow().as_slice(), &[true, false]);
    });

    drop(subscription);
    cx.update(|cx| cx.set_reduce_motion(true));
    assert_eq!(observed.borrow().as_slice(), &[true, false]);
}
