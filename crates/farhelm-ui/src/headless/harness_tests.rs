//! Self-tests for the harness itself, limited to behavior the ported tests
//! cannot show on their own.

use super::{Page, server::FakeHelm};
use dioxus::prelude::*;

static ORDER: GlobalSignal<Vec<u32>> = Signal::global(|| vec![1, 2, 3, 4]);

/// A keyed list whose order the test rewrites.
fn keyed_list() -> Element {
    rsx! {
        ul {
            for n in ORDER() {
                li { key: "{n}", "{n}" }
            }
        }
    }
}

/// Spec: after keyed reorders (a node moved from before its anchor, one
/// moved from after it, and a full reversal), the DOM lists the items in the
/// VirtualDom's new order.
///
/// Why: keyed diffing moves live nodes within one parent. An earlier version
/// of `Dom::place` took the insertion index before detaching the moved
/// node, which put a node moved from before its anchor one slot late; no
/// ported test reordered a keyed list, so nothing else would have caught it,
/// and every later query on that list would have read a wrong order.
#[farhelm_testtrace::test]
async fn keyed_moves_keep_the_virtual_dom_order() {
    let helm = FakeHelm::start().await;
    let mut page = Page::mount(&helm, keyed_list);
    let list = page.wait_for("ul").await;
    for order in [vec![2, 3, 1, 4], vec![2, 4, 3, 1], vec![1, 3, 4, 2]] {
        let expected: String = order.iter().map(u32::to_string).collect();
        page.vdom.in_runtime(|| *ORDER.write() = order.clone());
        page.wait_until(&format!("the list reading {expected}"), |p| {
            (p.text(list).replace(' ', "") == expected).then_some(())
        })
        .await;
    }
}

/// Spec: a wait that never succeeds fails at its deadline, naming what it
/// waited for and listing the requests the page made.
///
/// Why: the failure message is the harness's whole debugging story. A wait
/// that hung, or failed without saying what it wanted and what the page
/// asked the fake helm for, would turn every missing route into guesswork.
#[farhelm_testtrace::test]
#[should_panic(expected = "timed out waiting for something that never appears")]
async fn a_failed_wait_says_what_it_wanted() {
    let helm = FakeHelm::start().await;
    let mut page = Page::mount(&helm, keyed_list);
    page.set_wait(std::time::Duration::from_millis(200));
    page.wait_until("something that never appears", |_| None::<()>)
        .await;
}

/// Spec: `settle` fails when the page cannot be quiet for its 300 ms
/// within the wait deadline, here a 100 ms deadline on an idle page.
///
/// Why: `settle` is how a test claims "nothing else happened". If its
/// deadline only applied between bursts of work, a page that went quiet just
/// after the deadline (or a deadline shorter than the quiet period) would
/// pass the claim late instead of failing it.
#[farhelm_testtrace::test]
#[should_panic(expected = "the page never went quiet for 300 ms")]
async fn settle_fails_when_quiet_cannot_come_within_the_deadline() {
    let helm = FakeHelm::start().await;
    let mut page = Page::mount(&helm, keyed_list);
    page.set_wait(std::time::Duration::from_millis(100));
    page.settle().await;
}
