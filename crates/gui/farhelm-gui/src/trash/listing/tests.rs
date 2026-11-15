//! Tests of the trash rules that need no window: totals and their wording, which checkout an entry's Delete may clean,
//! what a delete's answer does to the kept listing and refusals, and the button's summary and help.

use std::rc::Rc;

use farhelm_proto::{
    ArchivedCheckout, CheckoutTrashDeleted, CheckoutTrashIssue, CheckoutTrashStuck,
};

use super::*;

/// A checkout `id` taking `bytes`, in `/work/.farhelm-trash`.
fn archive(id: &str, bytes: Option<u64>) -> ArchivedCheckout {
    ArchivedCheckout {
        id: id.into(),
        name: id.into(),
        repository: "acme/web".into(),
        path: format!("/work/.farhelm-trash/{id}"),
        archived_at: None,
        bytes,
        removing: false,
        stuck: None,
    }
}

/// A diagnostic the host could not check, for record `id`.
fn unchecked(id: &str) -> CheckoutTrashIssue {
    CheckoutTrashIssue {
        id: id.into(),
        path: "/work/.farhelm-trash/unreadable".into(),
        message: "could not check ownership".into(),
    }
}

/// A connected host `id`, named `name`, whose settled listing is `listing`.
fn shown(id: i64, name: &str, listing: CheckoutTrashListing) -> HostSnapshot {
    HostSnapshot {
        id,
        name: HostText::from_host(name),
        incarnation: 1,
        connected: true,
        listing: Some(Ok(Rc::new(listing))),
        pending: false,
        removing: false,
        notice_error: None,
        notices: Vec::new(),
        busy: false,
    }
}

/// Why this matters: an archive the host could not inspect or measure must never read as an exact zero, and repeated
/// diagnostics describe one entry (the old `host_totals_qualify_unchecked_entries_and_partial_sizes`). Specified: two
/// diagnostics of one record are one unchecked entry and make the size partial; a checkout with no size makes it
/// partial; a fully measured listing is exact; sizes print in binary units to one decimal, bytes as they are.
#[test]
fn totals_qualify_unchecked_entries_and_partial_sizes() {
    let cases = [
        (
            vec![],
            vec![unchecked("u"), unchecked("u")],
            "0 checkouts · 1 unchecked entry",
            "0 B measured · partial",
        ),
        (
            vec![archive("a", Some(4096))],
            vec![unchecked("u"), unchecked("v")],
            "1 checkout · 2 unchecked entries",
            "4.0 KiB measured · partial",
        ),
        (
            vec![archive("a", None)],
            vec![],
            "1 checkout",
            "0 B measured · partial",
        ),
        (
            vec![archive("a", Some(4096)), archive("b", Some(48_234_496))],
            vec![],
            "2 checkouts",
            "46.0 MiB",
        ),
    ];
    for (checkouts, issues, count, size) in cases {
        let found = totals(&CheckoutTrashListing {
            checkouts,
            issues,
            ..CheckoutTrashListing::default()
        });
        assert_eq!(found.count_label(), count);
        assert_eq!(space(found.bytes, found.partial), size);
    }
    assert_eq!(space(512, false), "512 B");
}

/// Why this matters: archive and failure times are the host's recorded UTC times, shown to the minute as the old
/// dialog did, and the folders to copy work out of must each be named once. Specified: a stamp prints as
/// `YYYY-MM-DD HH:MM UTC`; the folders are the distinct parents of every checkout's and diagnostic's path, sorted.
#[test]
fn stamps_are_utc_minutes_and_folders_are_each_parent_once() {
    assert_eq!(stamp(1_794_591_655), "2026-11-13 17:40 UTC");
    let listing = CheckoutTrashListing {
        checkouts: vec![archive("a", None), archive("b", None)],
        issues: vec![unchecked("u")],
        ..CheckoutTrashListing::default()
    };
    assert_eq!(folders(&listing), ["/work/.farhelm-trash"]);
}

/// Why this matters: an entry's Delete removes a checkout permanently with no question, so it must select only the row
/// pressed and only while the host's guards hold (SPEC.md: "unavailable under the same host-listing and operation
/// guards as bulk cleanup"), with the connection frozen so a reconnected host refuses it. Specified: a settled listing
/// selects the one checkout on its connection; a diagnostic-only record, an absent ID and a removing row select
/// nothing; a host being read, disconnected, busy with its own work or over the batch limit selects nothing.
#[test]
fn an_entry_delete_selects_one_listed_checkout_only_under_the_hosts_guards() {
    let mut removing = archive("removing", None);
    removing.removing = true;
    let host = shown(
        2,
        "build-box",
        CheckoutTrashListing {
            checkouts: vec![archive("a", None), archive("b", None), removing],
            issues: vec![unchecked("u")],
            ..CheckoutTrashListing::default()
        },
    );
    assert_eq!(
        host.entry_target("a"),
        Some(Target {
            host: 2,
            incarnation: 1,
            ids: vec!["a".into()]
        })
    );
    for id in ["u", "absent", "removing"] {
        assert_eq!(host.entry_target(id), None, "{id}");
    }
    let refused: [fn(&mut HostSnapshot); 3] = [
        |host| host.pending = true,
        |host| host.connected = false,
        |host| host.busy = true,
    ];
    for refuse in refused {
        let mut guarded = host.clone();
        refuse(&mut guarded);
        assert_eq!(guarded.entry_target("a"), None);
    }
    let mut over = host.clone();
    if let Some(Ok(listing)) = over.listing.as_mut() {
        Rc::make_mut(listing)
            .checkouts
            .extend((0..MAX_SELECTION).map(|at| archive(&format!("extra-{at}"), None)));
    }
    assert_eq!(over.entry_target("a"), None, "over the batch limit");
}

/// Why this matters: the rail notices panel's Retry now asks the host for another attempt on a checkout whose cleanup
/// failed, and SPEC.md makes it "unavailable without a current connected host listing showing that same checkout
/// stuck"; a Retry of a checkout the listing does not show stuck could remove one the user never saw fail. Specified: a
/// capable listing (`reports_retired`) showing the checkout stuck selects it as an entry's Delete would; the same
/// checkout not stuck, a listing that does not report retained removals, and the entry's own guards (a host being
/// read) select nothing.
#[test]
fn retry_needs_a_capable_listing_showing_the_checkout_stuck() {
    let mut stuck = archive("a", None);
    stuck.stuck = Some(CheckoutTrashStuck {
        reason: "Permission denied".into(),
        at: 1_794_591_900,
    });
    let listing = |checkout: ArchivedCheckout, capable: bool| CheckoutTrashListing {
        checkouts: vec![checkout],
        reports_retired: capable,
        ..CheckoutTrashListing::default()
    };
    let host = shown(2, "build-box", listing(stuck.clone(), true));
    assert_eq!(host.retry_target("a"), host.entry_target("a"));
    assert!(host.retry_target("a").is_some());
    assert_eq!(
        shown(2, "build-box", listing(archive("a", None), true)).retry_target("a"),
        None,
        "not stuck"
    );
    assert_eq!(
        shown(2, "build-box", listing(stuck, false)).retry_target("a"),
        None,
        "no retained removal records"
    );
    let mut reading = host.clone();
    reading.pending = true;
    assert_eq!(reading.retry_target("a"), None, "a read is out");
}

/// Why this matters: the bulk deletes freeze exactly what the user is shown, and Delete all must neither wait on a
/// host it cannot reach nor quietly leave out one it can (SPEC.md: "The global action skips unreachable hosts and
/// waits until every reachable host can be read"). Specified: a host's batch holds its checkouts' and its diagnostics'
/// IDs once each, on its connection, with its totals; Delete all takes one batch per reachable host holding something,
/// in registry order, skipping an unreachable host and an empty one; it offers nothing while the registry is unread,
/// while a reachable host is being read or is busy with its own work, or while one is over the batch limit.
#[test]
fn delete_all_batches_every_reachable_host_and_waits_on_any_unsettled_one() {
    let first = shown(
        1,
        "this machine",
        CheckoutTrashListing {
            checkouts: vec![archive("a", Some(1)), archive("u", None)],
            issues: vec![unchecked("u"), unchecked("v")],
            ..CheckoutTrashListing::default()
        },
    );
    let second = shown(
        2,
        "build-box",
        CheckoutTrashListing {
            checkouts: vec![archive("c", Some(2))],
            ..CheckoutTrashListing::default()
        },
    );
    let mut gone = second.clone();
    gone.id = 3;
    gone.connected = false;
    let empty = shown(4, "spare", CheckoutTrashListing::default());
    let snapshot = Snapshot {
        hosts_known: true,
        hosts: vec![first, second, gone, empty],
        ..Snapshot::default()
    };
    let Some(batches) = snapshot.global_batches() else {
        panic!("every reachable host is settled");
    };
    let targets: Vec<(i64, Vec<String>)> = batches
        .iter()
        .map(|batch| (batch.target.host, batch.target.ids.clone()))
        .collect();
    assert_eq!(
        targets,
        [
            (1, vec!["a".into(), "u".into(), "v".into()]),
            (2, vec!["c".into()]),
        ]
    );
    assert_eq!(batches[0].totals.unchecked, 2);

    let waits: [fn(&mut Snapshot); 4] = [
        |snapshot| snapshot.hosts_known = false,
        |snapshot| snapshot.hosts[1].pending = true,
        |snapshot| snapshot.hosts[1].busy = true,
        |snapshot| {
            if let Some(Ok(listing)) = snapshot.hosts[1].listing.as_mut() {
                Rc::make_mut(listing)
                    .checkouts
                    .extend((0..MAX_SELECTION).map(|at| archive(&format!("extra-{at}"), None)));
            }
        },
    ];
    for (at, wait) in waits.into_iter().enumerate() {
        let mut waiting = snapshot.clone();
        wait(&mut waiting);
        assert!(waiting.global_batches().is_none(), "case {at}");
    }
}

/// Why this matters: the question is the user's last look before work is lost for good, so it must name what goes,
/// where, and how much, and warn about unchecked entries as the old band did (SPEC.md: "naming the confirmed hosts,
/// checkout count and size, and warning that uncommitted or unpushed work is permanently lost"). Specified: the
/// checkouts of two hosts are counted together with their partial size and both hosts named, escaped, then the
/// warning; the unchecked entries name only the host that listed them; a batch of unchecked entries alone has no
/// checkout lines.
#[test]
fn the_confirmation_names_counts_size_and_hosts_and_warns() {
    let host = |id: i64, name: &str, checkouts: Vec<ArchivedCheckout>, issues| Batch {
        target: Target {
            host: id,
            incarnation: 1,
            ids: Vec::new(),
        },
        name: HostText::from_host(name),
        totals: totals(&CheckoutTrashListing {
            checkouts,
            issues,
            ..CheckoutTrashListing::default()
        }),
    };
    let both = [
        host(
            1,
            "this machine",
            vec![archive("a", Some(1024))],
            vec![unchecked("u")],
        ),
        host(
            2,
            "build\u{202e}box",
            vec![archive("c", Some(1024))],
            Vec::new(),
        ),
    ];
    let words = |batches: &[Batch]| -> Vec<String> {
        confirmation_text(batches)
            .iter()
            .map(|line| line.accessible_name().to_string())
            .collect()
    };
    assert_eq!(
        words(&both),
        [
            "Delete 2 archived checkouts (2.0 KiB measured · partial) on this machine, build<U+202E>box?",
            "Work in them that was never committed or pushed is lost permanently.",
            "Attempt cleanup of 1 unchecked entry on this machine.",
            "Farhelm deletes any of these it can verify when cleanup runs; work in them that was never committed or \
             pushed is lost permanently. The rest stay in place and may be forgotten from the trash.",
        ]
    );
    let only_unchecked = [host(
        1,
        "this machine",
        Vec::new(),
        vec![unchecked("u"), unchecked("v")],
    )];
    assert_eq!(
        words(&only_unchecked)[0],
        "Attempt cleanup of 2 unchecked entries on this machine."
    );
    assert_eq!(words(&only_unchecked).len(), 2);
}

/// Why this matters: a cleanup receipt is not proof the space is free (SPEC.md: "the row stays marked as removing until
/// a later listing confirms it is gone"), while an older host hides accepted rows itself and keeps its existing
/// disappearance. Specified: on a capable host the accepted row stays, removing, with its failure cleared, and the host
/// reports removal (which starts the rechecks), its neighbour untouched; on an older host the row goes and nothing is
/// reported; either way the accepted ID's diagnostics go.
#[test]
fn an_accepted_delete_keeps_a_capable_hosts_row_removing_until_a_later_listing() {
    for capable in [true, false] {
        let mut stuck = archive("a", Some(1));
        stuck.stuck = Some(CheckoutTrashStuck {
            reason: "permission denied".into(),
            at: 1,
        });
        let mut state = HostTrash {
            incarnation: 1,
            listing: Some(Ok(Rc::new(CheckoutTrashListing {
                checkouts: vec![stuck, archive("b", Some(1))],
                issues: vec![unchecked("a")],
                reports_retired: capable,
                ..CheckoutTrashListing::default()
            }))),
            ..HostTrash::default()
        };
        accepted_cleanup(&mut state, &["a".into()]);
        let Some(Ok(listing)) = &state.listing else {
            panic!("the listing stays a listing");
        };
        assert!(listing.issues.is_empty(), "capable {capable}");
        if capable {
            assert_eq!(listing.checkouts.len(), 2);
            assert!(listing.checkouts[0].removing && listing.checkouts[0].stuck.is_none());
            assert!(!listing.checkouts[1].removing);
            assert!(state.removing && listing.removing);
        } else {
            assert_eq!(listing.checkouts.len(), 1);
            assert_eq!(listing.checkouts[0].id, "b");
            assert!(!state.removing);
        }
    }
}

/// Why this matters: SPEC.md keeps refusals visible on the next opening, and the old dialog never let one checkout's
/// answer settle another's refusal (its test "deleting another trash entry preserves an earlier refusal"). Specified: a
/// host's refusal for `a` is kept with its path; an unconfirmed answer for `b` is kept beside it; a cleanup report
/// removing `c` leaves both; a second unconfirmed answer for `b` replaces the first; a report removing `b` settles
/// `b`'s; a report refusing `a` anew replaces `a`'s with the new words.
#[test]
fn a_delete_answer_settles_only_its_own_checkouts_notices() {
    let mut notices = Vec::new();
    let refusal = |id: &str, words: &str| CheckoutTrashDeleted {
        removed: Vec::new(),
        issues: vec![CheckoutTrashIssue {
            id: id.into(),
            path: format!("/work/.farhelm-trash/{id}"),
            message: words.into(),
        }],
    };
    let removed = |id: &str| CheckoutTrashDeleted {
        removed: vec![id.into()],
        issues: Vec::new(),
    };
    record_outcome(
        &mut notices,
        &["a".into()],
        &Ok(refusal("a", "a session uses this archive")),
    );
    record_outcome(
        &mut notices,
        &["b".into()],
        &Err(HostText::from_host("the host timed out")),
    );
    record_outcome(&mut notices, &["c".into()], &Ok(removed("c")));
    let ids = |notices: &[Notice]| -> Vec<(String, bool)> {
        notices
            .iter()
            .map(|notice| (notice.ids.join(","), notice.unconfirmed))
            .collect()
    };
    assert_eq!(ids(&notices), [("a".into(), false), ("b".into(), true)]);
    assert_eq!(
        notices[0].path.as_ref().map(HostText::raw),
        Some("/work/.farhelm-trash/a")
    );
    record_outcome(
        &mut notices,
        &["b".into()],
        &Err(HostText::from_host("lost again")),
    );
    assert_eq!(ids(&notices), [("a".into(), false), ("b".into(), true)]);
    assert_eq!(notices[1].message.raw(), "lost again");
    record_outcome(&mut notices, &["b".into()], &Ok(removed("b")));
    assert_eq!(ids(&notices), [("a".into(), false)]);
    record_outcome(
        &mut notices,
        &["a".into()],
        &Ok(refusal("a", "still in use")),
    );
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].message.raw(), "still in use");
}

/// Why this matters: a bulk delete whose answer is lost may have freed any of its folders, so the dialog must say so
/// once, for all of them, until a later answer settles each (the old app kept one uncertain record per delete, and its
/// "trash batch issues survive reopening" cases). Specified: an unconfirmed answer to a three-ID delete is one notice
/// naming all three; a report removing one and refusing another leaves the unconfirmed notice naming only the third,
/// beside the refusal; the report also naming `z`, which this delete never sent, leaves the earlier refusal for `z`.
#[test]
fn a_bulk_delete_answer_is_one_notice_that_later_reports_shrink() {
    let sent: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    let mut notices = vec![Notice {
        ids: vec!["z".into()],
        path: None,
        message: HostText::from_host("a session uses this archive"),
        unconfirmed: false,
    }];
    record_outcome(
        &mut notices,
        &sent,
        &Err(HostText::from_host("the host timed out")),
    );
    assert_eq!(notices.len(), 2);
    assert_eq!(notices[1].ids, sent);
    assert!(notices[1].unconfirmed);
    record_outcome(
        &mut notices,
        &sent,
        &Ok(CheckoutTrashDeleted {
            removed: vec!["a".into(), "z".into()],
            issues: vec![CheckoutTrashIssue {
                id: "b".into(),
                path: String::new(),
                message: "in use".into(),
            }],
        }),
    );
    let kept: Vec<(Vec<String>, bool)> = notices
        .iter()
        .map(|notice| (notice.ids.clone(), notice.unconfirmed))
        .collect();
    assert_eq!(
        kept,
        [
            (vec!["z".into()], false),
            (vec!["c".into()], true),
            (vec!["b".into()], false),
        ]
    );
}

/// Why this matters: the button's name and help are all a user sees of the trash without opening it: what it holds,
/// whether the count is complete, which hosts free space while the can is over (SPEC.md: "Its tooltip names the hosts
/// freeing space"), and that a refusal waits in the dialog ("the trash button's tooltip tells the user to open the trash
/// to see why"). Specified: an empty settled trash and a full one read as in the old app; an unanswered host makes the
/// count known only; a host freeing space is named, escaped, and an unreachable one never; a delete in flight alone
/// says it empties; a kept refusal adds the pointer into the dialog.
#[test]
fn the_summary_and_help_say_what_the_trash_holds_and_who_frees_space() {
    let empty = Snapshot {
        hosts_known: true,
        hosts: vec![shown(1, "this machine", CheckoutTrashListing::default())],
        ..Snapshot::default()
    };
    assert_eq!(empty.summary(), (false, "empty".to_owned()));
    assert_eq!(
        empty.help().accessible_name().as_ref(),
        "Trash: empty. When you delete the last session using a managed checkout, its folder moves here instead of \
         being deleted."
    );

    let full_listing = CheckoutTrashListing {
        checkouts: vec![archive("a", Some(1)), archive("b", Some(1))],
        removing: true,
        ..CheckoutTrashListing::default()
    };
    let mut freeing = shown(2, "build\u{202e}box", full_listing);
    freeing.removing = true;
    let mut gone = shown(3, "gpu-1", CheckoutTrashListing::default());
    gone.connected = false;
    gone.removing = true;
    let mut full = Snapshot {
        hosts_known: true,
        hosts: vec![empty.hosts[0].clone(), freeing, gone],
        ..Snapshot::default()
    };
    assert_eq!(full.summary(), (true, "2 archived checkouts".to_owned()));
    assert!(full.emptying());
    assert_eq!(
        full.help().accessible_name().as_ref(),
        "Emptying the trash: freeing space on build<U+202E>box. Trash: 2 archived checkouts. A managed checkout moves \
         here when the last session using it is deleted. Click to see them, or to delete them for good."
    );

    full.hosts[1].notices.push(Notice {
        ids: vec!["a".into()],
        path: None,
        message: HostText::from_host("refused"),
        unconfirmed: false,
    });
    assert!(
        full.help()
            .accessible_name()
            .ends_with(" Some checkouts could not be deleted. Open the trash to see why.")
    );

    let mut waiting = empty.clone();
    waiting.hosts[0].pending = true;
    waiting.hosts[0].listing = None;
    waiting.delete_in_flight = true;
    assert_eq!(
        waiting.summary(),
        (
            false,
            "0 known archived checkouts, some entries or hosts unchecked".to_owned()
        )
    );
    assert_eq!(
        waiting.help().accessible_name().as_ref(),
        "Emptying the trash. Trash: 0 known archived checkouts, some entries or hosts unchecked. Click to see what \
         is known."
    );
}
