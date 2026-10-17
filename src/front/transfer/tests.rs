use super::*;

fn git(code: u8) -> Outcome {
    Outcome::Git(End::Code(code))
}

fn push(refused: Option<Refused>, given: Option<&str>) -> Then {
    Then::Push {
        refused,
        given: given.map(|given| given.as_bytes().to_vec()),
    }
}

fn origin() -> Vec<u8> {
    b"origin".to_vec()
}

/// The hint, where no question may be asked.
fn unasked(then: &Then, ended: &Outcome) -> Option<Vec<u8>> {
    hint(then, ended, || panic!("asked for {then:?}"))
}

/// The hint, where the question is asked once and answers `answer`.
fn answered(then: &Then, ended: &Outcome, answer: Answer) -> Option<Vec<u8>> {
    let mut asked = 0;
    let hint = hint(then, ended, || {
        asked += 1;
        answer
    });
    assert_eq!(asked, 1, "{then:?}");
    hint
}

#[test]
fn the_no_remote_hint_follows_a_run_that_did_not_exit_0() {
    let then = Then::NoRemoteChosen;
    assert_eq!(unasked(&then, &git(0)), None);
    for ended in [git(128), git(1), Outcome::Git(End::Signal(9))] {
        assert_eq!(unasked(&then, &ended).as_deref(), Some(NO_REMOTE_CHOSEN));
    }
    // A run that did not start, or a refusal, is no run.
    assert_eq!(unasked(&then, &Outcome::Refused), None);
    for ended in [git(0), git(1), git(128), Outcome::Git(End::Signal(9))] {
        assert_eq!(unasked(&Then::Nothing, &ended), None);
    }
}

/// After 128, the hint for what Git refused the push for, `HEAD` asked about only for a
/// branch with no upstream; after 1, the rejected hint naming the destination given; after
/// any other end, none, and no question (G22).
#[test]
fn each_push_hint_follows_its_end_and_asks_only_where_the_upstream_hint_is_due() {
    let no_upstream_given = |given| push(Some(Refused::NoUpstream { remote: origin() }), given);
    for given in [None, Some("origin")] {
        let then = no_upstream_given(given);
        assert_eq!(
            answered(&then, &git(128), Answer::Yes),
            Some(no_upstream(b"origin"))
        );
        assert_eq!(
            answered(&then, &git(128), Answer::No),
            Some(unborn(b"origin"))
        );
        // A question that answers neither gives no hint, and no other hint follows.
        assert_eq!(answered(&then, &git(128), Answer::Unanswered), None);
    }
    let other_name = Refused::OtherName {
        remote: origin(),
        merge: b"refs/heads/main".to_vec(),
    };
    let detached = Refused::Detached {
        remote: b"backup".to_vec(),
    };
    for (refused, after_128) in [
        (
            Some(other_name.clone()),
            Some(super::other_name(b"origin", b"refs/heads/main")),
        ),
        (Some(detached.clone()), Some(super::detached(b"backup"))),
        (None, None),
    ] {
        for given in [None, Some("origin")] {
            let then = push(refused.clone(), given);
            assert_eq!(unasked(&then, &git(128)), after_128, "{then:?}");
        }
    }
    for refused in [
        None,
        Some(Refused::NoUpstream { remote: origin() }),
        Some(other_name),
        Some(detached),
    ] {
        let given_none = push(refused.clone(), None);
        assert_eq!(
            unasked(&given_none, &git(1)).as_deref(),
            Some(hints::REJECTED)
        );
        let given = push(refused.clone(), Some("backup"));
        assert_eq!(unasked(&given, &git(1)), Some(rejected(Some(b"backup"))));
        for then in [&given_none, &given] {
            for (at, ended) in [
                git(0),
                git(129),
                git(2),
                Outcome::Git(End::Signal(1)),
                Outcome::Git(End::Signal(128)),
                Outcome::Refused,
            ]
            .iter()
            .enumerate()
            {
                assert_eq!(unasked(then, ended), None, "{then:?} after end {at}");
            }
        }
    }
    // The no-remote hint and the rejected one are never both owed.
    assert_eq!(
        unasked(&Then::NoRemoteChosen, &git(1)).as_deref(),
        Some(NO_REMOTE_CHOSEN)
    );
}

#[test]
fn a_pull_hints_at_its_upstream_first_and_at_reconciling_after_128_alone() {
    let pull = |untracked, unreconciled| Then::Pull {
        untracked,
        unreconciled,
    };
    for unreconciled in [false, true] {
        let then = pull(true, unreconciled);
        for ended in [git(1), git(128), Outcome::Git(End::Signal(2))] {
            assert_eq!(unasked(&then, &ended).as_deref(), Some(NOT_TRACKING));
        }
        assert_eq!(unasked(&then, &git(0)), None);
    }
    assert_eq!(
        unasked(&pull(false, true), &git(128)).as_deref(),
        Some(UNRECONCILED)
    );
    for ended in [
        git(0),
        git(1),
        Outcome::Git(End::Signal(128)),
        Outcome::Refused,
    ] {
        assert_eq!(unasked(&pull(false, true), &ended), None);
    }
    assert_eq!(unasked(&pull(false, false), &git(128)), None);
}

#[test]
fn the_reconciling_words_are_read_exactly() {
    for word in [
        "--rebase",
        "--rebase=false",
        "--rebase=merges",
        "-r",
        "--no-rebase",
        "--ff",
        "--no-ff",
        "--ff-only",
    ] {
        assert!(reconciling(word.as_bytes()), "{word}");
    }
    for word in [
        "--reb",
        "-rmerges",
        "-qr",
        "--ff-o",
        "--rebased",
        "origin",
        "--",
    ] {
        assert!(!reconciling(word.as_bytes()), "{word}");
    }
}
