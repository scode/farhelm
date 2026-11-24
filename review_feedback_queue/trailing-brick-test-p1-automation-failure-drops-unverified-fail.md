# P1 automation-failure path drops unverified product-failure claim if record() fails

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

During a release brick test, if a P1 check hits a guest automation failure after reporting a product failure and the
host import then fails (for example because the guest event log exceeded the 96 MiB export limit and was omitted from
the archive), the coordinator discards the unverified product-failure claim instead of flagging the run as
promotion_blocked_pending_review.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/brick_test.py:1049`.

In releasing/mac-vm-test/brick_test.py:1047-1055 (Run.test_owned), when a P1 checkpoint finishes with
execution.get('status') == 'automation-failure' and (observation or {}).get('status') == 'fail' (for check_id !=
'old-sessions'), the code appends the claim to entry['unverified_failures'] in memory and immediately calls
self.record(pass_id, check_id, 'could-not-run', [], ..., native=native, in_flight=True) without calling self.save()
first:

    with self.locked():
        entry = self.selected(pass_id)
        if (observation or {}).get('status') == 'fail' and check_id != 'old-sessions':
            entry.setdefault('unverified_failures', []).append(
                {key: observation[key] for key in ('identity', 'status', 'detail')})
        self.record(pass_id, check_id, 'could-not-run', [], ..., native=native, in_flight=True)

If self.record() raises before reaching its own self.save() — for example, when guest_tester.py:728-730 omitted an
oversized (>96 MiB) events.jsonl from the export archive so artifact(self.directory, native['events']) at
brick_test.py:758 raises KeyError, or any other Refused/archive error during import — control jumps to except
BaseException at brick_test.py:1109. The except handler re-enters with self.locked(), which reloads self.data from disk
(brick_test.py:597), discarding the unsaved entry['unverified_failures'] append. Unlike the completed-fail path at
brick_test.py:1092-1099 (which persists entry['tester']['observation'] via self.save() before calling self.record()) and
the non-P1 automation-failure path at brick_test.py:1058-1063 (which persists claim['observation'] via self.save()
before raising), neither unverified_failures nor entry['tester']['observation'] is saved. As a result, report()
(brick_test.py:1783-1791) produces unverified_product_failures: [] and promotion_blocked_pending_review: false,
violating BRICK-TEST.md ('If an aborted turn nevertheless reported a product failure, or a completed turn reported one
whose evidence then failed host import, unverified_product_failures surfaces that observation at run level and in its
tier's block').

Verification: Unit test a P1 checkpoint returning execution status 'automation-failure' with unverified_result status
'fail' and an archive missing events.jsonl (matching guest_tester.py's >96 MiB omission behavior). Observe that test()
raises KeyError/Refused and report()['promotion_blocked_pending_review'] is False and
report()['unverified_product_failures'] is [].

Fix: Call self.save() immediately after appending to entry['unverified_failures'] (before calling self.record()),
matching lines 1097-1099.

Reviewer metadata (g02_brick_test_coord_p1/f1): suggested bucket: other (harm is confined to maintainer release-testing
tooling and requires both a P1 automation-failure with an unverified fail observation and a subsequent record() import
failure; does not cause catastrophic harm or affect shipped user data).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g02_brick_test_coord_p1/f1`.
