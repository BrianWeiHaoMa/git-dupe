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
    hint(then, ended, |question| {
        panic!("asked {question:?} for {then:?}")
    })
}

/// The hint, where `question` is asked once and answers `answer`.
fn answered(then: &Then, ended: &Outcome, question: Question, answer: Answer) -> Option<Vec<u8>> {
    let mut asked = 0;
    let hint = hint(then, ended, |put| {
        assert_eq!(put, question, "{then:?}");
        asked += 1;
        answer
    });
    assert_eq!(asked, 1, "{then:?}");
    hint
}

fn pull(unnamed: Option<Unnamed>, diverging: Option<Diverging>) -> Then {
    Then::Pull { unnamed, diverging }
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
            answered(&then, &git(128), Question::HeadVerifies, Answer::Yes),
            Some(no_upstream(b"origin"))
        );
        assert_eq!(
            answered(&then, &git(128), Question::HeadVerifies, Answer::No),
            Some(unborn(b"origin"))
        );
        // A question that answers neither gives no hint, and no other hint follows.
        assert_eq!(
            answered(&then, &git(128), Question::HeadVerifies, Answer::Unanswered),
            None
        );
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

/// In the table's order, the first that holds: a branch with no upstream after 1, an
/// unchosen way after 128, a detached `HEAD` after 1, fast-forward only after 128; after 0
/// alone, the one question.
#[test]
fn a_pull_hints_in_the_tables_order_the_first_that_holds() {
    // Given a URL or a path, Git has the remote's `HEAD` to merge, and its advice after 128
    // is the divergent-branches or the fast-forward-only one.
    let others = [
        git(2),
        git(129),
        Outcome::Git(End::Signal(1)),
        Outcome::Refused,
    ];
    for (unnamed, line) in [
        (Unnamed::NoUpstream, NOT_TRACKING),
        (Unnamed::Detached, DETACHED_PULL),
    ] {
        for (diverging, after_128) in [
            (None, None),
            (Some(Diverging::Unchosen), Some(UNRECONCILED)),
            (Some(Diverging::FastForwardOnly), Some(FAST_FORWARD_ONLY)),
        ] {
            let then = pull(Some(unnamed), diverging);
            assert_eq!(unasked(&then, &git(1)).as_deref(), Some(line), "{then:?}");
            assert_eq!(unasked(&then, &git(128)).as_deref(), after_128, "{then:?}");
            for ended in &others {
                assert_eq!(unasked(&then, ended), None, "{then:?}");
            }
        }
    }
    for (diverging, line) in [
        (Diverging::Unchosen, UNRECONCILED),
        (Diverging::FastForwardOnly, FAST_FORWARD_ONLY),
    ] {
        let then = pull(None, Some(diverging));
        assert_eq!(unasked(&then, &git(128)).as_deref(), Some(line));
        // Exit 1 with no rebase stopped, a signal numbered 128, or no run: none.
        let not_stopped = [(Question::RebaseStopped, Answer::No)];
        assert_eq!(answering(&then, &git(1), &not_stopped), None);
        for ended in [git(129), Outcome::Git(End::Signal(128)), Outcome::Refused] {
            assert_eq!(unasked(&then, &ended), None, "{then:?}");
        }
    }
    for ended in [git(128), git(2), Outcome::Git(End::Signal(2))] {
        assert_eq!(unasked(&pull(None, None), &ended), None);
    }
}

/// The hint, where every question put is one of `answers`, in their order, each once.
fn answering(then: &Then, ended: &Outcome, answers: &[(Question, Answer)]) -> Option<Vec<u8>> {
    let mut put = answers.iter();
    let hint = hint(then, ended, |question| {
        let (expected, answer) = put.next().expect("asked more than `answers`");
        assert_eq!(question, *expected, "{then:?}");
        *answer
    });
    assert!(put.next().is_none(), "{then:?}: left unasked");
    hint
}

/// A rebase a `pull` left stopped draws its line where nothing is unmerged: after exit 0 an
/// `edit` or a `break`, no conflict, and the index is not asked; after exit 1 the index
/// tells a conflict, which draws none, from a failed reword or an unreadable todo list.
#[test]
fn a_pull_left_stopped_draws_the_rebases_line_whatever_the_index_holds() {
    let then = pull(None, None);
    let stopped = (Question::RebaseStopped, Answer::Yes);
    // The stop decides at either end: nothing is asked of the index.
    for end in [git(0), git(1)] {
        assert_eq!(
            answering(&then, &end, &[stopped]).as_deref(),
            Some(advice::REBASE)
        );
    }
    // No rebase stopped, after exit 1: nothing more is asked, and no hint.
    let none = (Question::RebaseStopped, Answer::No);
    assert_eq!(answering(&then, &git(1), &[none]), None);
    let unanswered = (Question::RebaseStopped, Answer::Unanswered);
    assert_eq!(answering(&then, &git(1), &[unanswered]), None);
}

/// After a `pull` that exited 0 with no rebase stopped, whatever else the guard decided, the
/// index is asked once, and an unmerged entry, which only an autostash that came back with
/// conflicts leaves there, draws the autostash's line; nothing is asked after any other end.
#[test]
fn a_pull_that_exited_0_asks_about_unmerged_entries_once() {
    for then in [
        pull(None, None),
        pull(Some(Unnamed::NoUpstream), Some(Diverging::Unchosen)),
        pull(Some(Unnamed::Detached), Some(Diverging::FastForwardOnly)),
    ] {
        let unmerged = |answer| {
            let not_stopped = (Question::RebaseStopped, Answer::No);
            answering(&then, &git(0), &[not_stopped, (Question::Unmerged, answer)])
        };
        assert_eq!(unmerged(Answer::Yes).as_deref(), Some(advice::AUTOSTASH));
        assert_eq!(unmerged(Answer::No), None);
        assert_eq!(unmerged(Answer::Unanswered), None);
    }
    for ended in [git(128), Outcome::Git(End::Signal(9)), Outcome::Refused] {
        assert_eq!(unasked(&pull(None, None), &ended), None);
    }
    // Neither `push` nor any other command asks it.
    for then in [
        Then::Nothing,
        Then::NoRemoteChosen,
        push(None, None),
        push(Some(Refused::NoUpstream { remote: origin() }), None),
    ] {
        assert_eq!(unasked(&then, &git(0)), None, "{then:?}");
    }
}

fn records<'a>(listing: &[(&'a str, Option<&'a str>)]) -> Vec<Record<'a>> {
    listing
        .iter()
        .map(|&(key, value)| (key.as_bytes(), value.map(str::as_bytes)))
        .collect()
}

#[test]
fn a_pull_lacks_a_branch_to_merge_on_a_detached_head_or_a_branch_with_no_upstream() {
    let main = Current::Branch(b"main");
    let upstream = [
        ("branch.main.remote", Some("origin")),
        ("branch.main.merge", Some("refs/heads/main")),
    ];
    let remote_alone = [("branch.main.remote", Some("origin"))];
    let unnamed_by = |listing, given, current| unnamed(&records(listing), given, current);
    for listing in [&[][..], &upstream, &remote_alone] {
        // Detached, given no repository or a remote word, whichever: no branch key.
        for given in [Repository::Default, Repository::Alone(b"backup")] {
            assert_eq!(
                unnamed_by(listing, given, Current::Detached),
                Some(Unnamed::Detached)
            );
        }
        // A refspec names what to merge; `HEAD` not read decides nothing.
        for current in [main, Current::Detached, Current::AnyBranch] {
            assert_eq!(
                unnamed_by(listing, Repository::WithRefspec(b"origin"), current),
                None
            );
        }
        assert_eq!(
            unnamed_by(listing, Repository::Alone(b"origin"), Current::AnyBranch),
            None
        );
    }
    // On a branch, `pull` counts the `merge` key alone.
    assert_eq!(unnamed_by(&upstream, Repository::Default, main), None);
    for listing in [&[][..], &remote_alone] {
        assert_eq!(
            unnamed_by(listing, Repository::Default, main),
            Some(Unnamed::NoUpstream)
        );
    }
    assert_eq!(
        unnamed_by(&remote_alone, Repository::Alone(b"origin"), main),
        Some(Unnamed::NoUpstream)
    );
    // Another remote than the branch's, or none set: Git's advice names no command.
    assert_eq!(
        unnamed_by(&remote_alone, Repository::Alone(b"backup"), main),
        None
    );
    assert_eq!(unnamed_by(&[], Repository::Alone(b"origin"), main), None);
}

#[test]
fn a_pull_is_told_to_fast_forward_only_by_its_word_or_by_pull_ff_only_untyped_over() {
    let main = Current::Branch(b"main");
    let diverging_by = |listing, given, words: &[&str]| {
        let words: Vec<&[u8]> = words.iter().map(|word| word.as_bytes()).collect();
        diverging(&records(listing), given, main, &words)
    };
    let only = [("pull.ff", Some("only"))];
    let rebase = [("pull.rebase", Some("true"))];
    let given = [
        Repository::Default,
        Repository::Alone(b"origin"),
        Repository::WithRefspec(b"origin"),
    ];
    for given in given {
        for (listing, words) in [
            // `--ff-only` typed decides, whatever else is typed or set.
            (&[][..], &["--ff-only"][..]),
            (&rebase, &["--ff-only"]),
            (&[], &["--rebase", "--ff-only"]),
            (&[], &["-o", "--ff-only"]),
            // The key, where no word of the six overrides it.
            (&only, &[]),
            (&only, &["-v", "--autostash"]),
            (&only, &["--ff-o"]),
            (
                &[("pull.ff", Some("false")), ("pull.ff", Some("only"))],
                &[],
            ),
            (
                &[("pull.ff", Some("only")), ("pull.rebase", Some("true"))],
                &[],
            ),
        ] {
            assert_eq!(
                diverging_by(listing, given, words),
                Some(Diverging::FastForwardOnly),
                "{listing:?} {words:?}"
            );
        }
        for (listing, words) in [
            // A word of the six overrides the key.
            (&only[..], &["--rebase"][..]),
            (&only, &["-r"]),
            (&only, &["--rebase=merges"]),
            (&only, &["--no-rebase"]),
            (&only, &["--ff"]),
            (&only, &["--no-ff"]),
            // A value Git refuses, `true`, or the key set aside, chooses another way.
            (&[("pull.ff", Some("Only"))], &[]),
            (&[("pull.ff", None)], &[]),
            (&[("pull.ff", Some("only")), ("pull.ff", Some("true"))], &[]),
            (&rebase, &[]),
            (&[], &["--no-rebase"]),
        ] {
            assert_eq!(
                diverging_by(listing, given, words),
                None,
                "{listing:?} {words:?}"
            );
        }
        // Nothing chooses: the divergent-branches hint.
        for words in [&[][..], &["-v"], &["--ff-o"], &["--autostash"]] {
            assert_eq!(
                diverging_by(&[], given, words),
                Some(Diverging::Unchosen),
                "{words:?}"
            );
        }
    }
    // Given a repository, any branch's `branch.<name>.rebase` chooses.
    let other = [("branch.other.rebase", Some("true"))];
    assert_eq!(
        diverging_by(&other, Repository::Default, &[]),
        Some(Diverging::Unchosen)
    );
    assert_eq!(
        diverging_by(&other, Repository::Alone(b"origin"), &[]),
        None
    );
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
