use super::*;

fn words(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}

fn word_of(words: &[&str]) -> Option<&'static [u8]> {
    admitted(&self::words(words)).map(|family| family.word)
}

/// An `ask` for a decision that must put no question.
fn unasked(question: Question) -> Answer {
    panic!("asked {question:?}")
}

#[test]
fn a_word_not_in_the_table_admits_no_hint() {
    for typed in [
        &["log"][..],
        &["stash", "pop"],
        &["Checkout", "HEAD~1"],
        &["merge", "side"],
        &["commit"],
        &[],
    ] {
        assert_eq!(word_of(typed), None, "{typed:?}");
    }
}

#[test]
fn the_words_admit_or_keep_away_each_family() {
    let admitting: [&[&str]; 12] = [
        &["rebase", "side"],
        &["rebase", "--continue"],
        &["am", "patch"],
        &["cherry-pick", "side"],
        &["revert", "HEAD~1"],
        &["checkout", "HEAD~1"],
        // `-d` is `switch`'s, not `checkout`'s; after `--` a word is a path.
        &["checkout", "-d"],
        &["checkout", "--", "--detach"],
        &["switch", "--", "-q"],
        &["rebase", "--", "-h"],
        &["branch", "-d", "side"],
        &["branch", "--delete", "side"],
    ];
    for typed in admitting {
        assert_eq!(word_of(typed), Some(typed[0].as_bytes()), "{typed:?}");
    }
    let kept_away: [&[&str]; 13] = [
        &["checkout", "--detach", "main"],
        &["checkout", "-q", "HEAD~1"],
        &["checkout", "--quiet", "HEAD~1"],
        &["switch", "--detach", "HEAD~1"],
        &["switch", "-d", "HEAD~1"],
        &["switch", "-q", "main"],
        &["rebase", "-h"],
        &["checkout", "--help"],
        &["cherry-pick", "x", "-h", "--", "y"],
        &["branch", "side"],
        &["branch", "-D", "side"],
        &["branch"],
        &["branch", "-d", "x", "-h"],
    ];
    for typed in kept_away {
        assert_eq!(word_of(typed), None, "{typed:?}");
    }
    // Exact words only: a bundle or an abbreviation is not read.
    for typed in [
        &["checkout", "-qf", "HEAD~1"][..],
        &["switch", "--det", "x"],
    ] {
        assert!(word_of(typed).is_some(), "{typed:?}");
    }
    assert_eq!(word_of(&["branch", "-dr", "x"]), None);
}

#[test]
fn a_detached_head_is_asked_about_before_the_run_only_where_the_words_admit_it() {
    let typed = words(&["checkout", "HEAD~1"]);
    for (before, admitted) in [
        (Answer::Yes, true),
        // Already detached: Git prints no advice for a move between commits.
        (Answer::No, false),
        (Answer::Unanswered, false),
    ] {
        let mut asked = Vec::new();
        let found = before_asking(&typed, |question| {
            asked.push(question);
            before
        });
        assert_eq!(found.is_some(), admitted, "{before:?}");
        assert_eq!(asked, [Question::OnABranch]);
    }
    // Words that keep the hint away, and every other family, put no question first.
    for (typed, admitted) in [
        (&["checkout", "--detach", "main"][..], false),
        (&["switch", "-q", "-d", "HEAD~1"], false),
        (&["log"], false),
        (&["rebase", "side"], true),
        (&["cherry-pick", "side"], true),
        (&["branch", "-d", "side"], true),
    ] {
        let found = before_asking(&words(typed), unasked);
        assert_eq!(found.is_some(), admitted, "{typed:?}");
    }
}

#[test]
fn each_trigger_follows_its_end_and_state() {
    let picking = Trigger::Picking("CHERRY_PICK_HEAD");
    for trigger in [Trigger::Stopped, picking] {
        for end in [End::Code(1), End::Code(128), End::Signal(9)] {
            assert!(
                follows(trigger, &end, |_| Answer::Yes),
                "{trigger:?} {end:?}"
            );
            assert!(
                !follows(trigger, &end, |_| Answer::No),
                "{trigger:?} {end:?}"
            );
            assert!(!follows(trigger, &end, |_| Answer::Unanswered));
        }
        assert!(!follows(trigger, &End::Code(0), unasked), "{trigger:?}");
    }
    assert!(follows(Trigger::Stopped, &End::Code(1), |question| {
        assert_eq!(question, Question::Stopped);
        Answer::Yes
    }));
    assert!(follows(picking, &End::Code(1), |question| {
        assert_eq!(question, Question::Verifies("CHERRY_PICK_HEAD"));
        Answer::Yes
    }));
    assert!(follows(Trigger::ExitedOne, &End::Code(1), unasked));
    for end in [End::Code(0), End::Code(128), End::Signal(1), End::Signal(9)] {
        assert!(!follows(Trigger::ExitedOne, &end, unasked), "{end:?}");
    }
}

#[test]
fn a_detached_head_follows_a_run_that_exited_0_and_symbolic_ref_answering_1() {
    // The question after the run: exit 0, on a branch; exit 1, detached; 128, a
    // question that failed.
    for (after, hinted) in [
        (Answer::Yes, false),
        (Answer::No, true),
        (Answer::Unanswered, false),
    ] {
        assert_eq!(
            follows(Trigger::Detached, &End::Code(0), |question| {
                assert_eq!(question, Question::OnABranch);
                after
            }),
            hinted,
            "{after:?}"
        );
    }
    // A signal is never exit 0.
    for end in [
        End::Code(1),
        End::Code(128),
        End::Signal(9),
        End::Signal(15),
    ] {
        assert!(!follows(Trigger::Detached, &end, unasked), "{end:?}");
    }
}

#[test]
fn each_hint_names_the_git_dupe_commands_and_the_plain_ones_project() {
    let named: [(&[u8], &[&str]); 6] = [
        (
            REBASE,
            &[
                "'git dupe add <path>'",
                "'git dupe rebase --continue'",
                "'git dupe rebase --skip'",
                "'git dupe rebase --abort'",
            ],
        ),
        (
            AM,
            &[
                "'git dupe add <path>'",
                "'git dupe am --continue'",
                "'git dupe am --skip'",
                "'git dupe am --abort'",
            ],
        ),
        (
            CHERRY_PICK,
            &[
                "'git dupe add <path>'",
                "'git dupe commit --allow-empty'",
                "'git dupe cherry-pick --continue'",
                "'git dupe cherry-pick --skip'",
                "'git dupe cherry-pick --abort'",
            ],
        ),
        (
            REVERT,
            &[
                "'git dupe add <path>'",
                "'git dupe revert --continue'",
                "'git dupe revert --skip'",
                "'git dupe revert --abort'",
            ],
        ),
        (
            DETACHED,
            &[
                "'git dupe switch -c <new-branch-name>'",
                "'git dupe switch -'",
            ],
        ),
        (UNMERGED, &["'git dupe branch -D <branch>'"]),
    ];
    for (line, commands) in named {
        let line = String::from_utf8(line.to_vec()).unwrap();
        for command in commands {
            assert!(line.contains(command), "{line} names no {command}");
        }
        assert!(line.contains("act"), "{line}");
        assert!(line.ends_with("on the project's repository"), "{line}");
        assert!(!line.contains('\n') && !line.contains("  "), "{line}");
    }
    for family in &FAMILIES {
        assert!(named.iter().any(|(line, _)| *line == family.line));
    }
}
