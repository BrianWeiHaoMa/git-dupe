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
    let admitting: [&[&str]; 22] = [
        &["rebase", "side"],
        &["rebase", "--continue"],
        &["am", "patch"],
        &["cherry-pick", "side"],
        &["revert", "HEAD~1"],
        &["checkout", "HEAD~1"],
        // `-d` is `switch`'s, not `checkout`'s; after `--` a word is a path or a revision.
        &["checkout", "-d"],
        &["checkout", "--", "--detach"],
        &["switch", "--", "-q"],
        &["rebase", "--", "-h"],
        &["cherry-pick", "--", "-n"],
        &["revert", "x", "--", "--no-commit"],
        &["branch", "-d", "side"],
        &["branch", "--delete", "side"],
        // Every spelling Git reads as `--delete`, or refuses as ambiguous with exit 129.
        &["branch", "-vd", "side"],
        &["branch", "-dv", "side"],
        &["branch", "-dr", "x"],
        &["branch", "-fd", "x"],
        &["branch", "--del", "x"],
        &["branch", "--dele", "x"],
        &["branch", "--d", "x"],
        &["branch", "x", "-d"],
    ];
    for typed in admitting {
        assert_eq!(word_of(typed), Some(typed[0].as_bytes()), "{typed:?}");
    }
    let kept_away: [&[&str]; 28] = [
        &["checkout", "--detach", "main"],
        &["checkout", "-q", "HEAD~1"],
        &["checkout", "--quiet", "HEAD~1"],
        &["switch", "--detach", "HEAD~1"],
        &["switch", "-d", "HEAD~1"],
        &["switch", "-q", "main"],
        &["rebase", "-h"],
        &["checkout", "--help"],
        &["cherry-pick", "x", "-h", "--", "y"],
        // Under `-n` or `--no-commit` Git's advice names only `git add` and `git rm`.
        &["cherry-pick", "-n", "x"],
        &["cherry-pick", "--no-commit", "x"],
        &["revert", "-n", "x"],
        &["revert", "--no-commit", "x"],
        &["revert", "x", "--no-commit", "--", "y"],
        &["revert", "-sn", "x"],
        &["cherry-pick", "-xn", "x"],
        &["branch", "side"],
        &["branch", "-D", "side"],
        &["branch"],
        &["branch", "-d", "x", "-h"],
        // `D` forces the deletion, in a bundle too; `=` or a longer word is no prefix.
        &["branch", "-Dd", "x"],
        &["branch", "-dD", "x"],
        &["branch", "--de=x"],
        &["branch", "--deleted", "x"],
        &["branch", "--delete=x"],
        &["branch", "--", "x"],
        // After `--` a word is a branch name.
        &["branch", "--", "-d"],
        &["branch", "x", "--", "--delete"],
    ];
    for typed in kept_away {
        assert_eq!(word_of(typed), None, "{typed:?}");
    }
    // A word that keeps the detached hint away is read exactly: a bundle or an abbreviation
    // of it is not. A prefix of `--no-commit` is never read as it.
    for typed in [
        &["checkout", "-qf", "HEAD~1"][..],
        &["switch", "--det", "x"],
        &["cherry-pick", "--no-c", "x"],
        &["revert", "--no-com", "x"],
    ] {
        assert!(word_of(typed).is_some(), "{typed:?}");
    }
}

#[test]
fn a_pick_reads_as_no_commit_the_word_and_n_in_a_bundle() {
    for word in [
        "--no-commit",
        "-n",
        "-en",
        "-ne",
        "-sn",
        "-xn",
        "-snx",
        "-nm1",
        "-nXours",
    ] {
        assert!(reads_as_no_commit(word.as_bytes()), "{word}");
    }
    // After `m`, `X`, or `S` the rest of the word is that option's value.
    for word in [
        "-m1",
        "-mn",
        "-Xn",
        "-Sn",
        "-sSn",
        "--no-c",
        "--no-com",
        "--no-commit=x",
        "--n",
        "---n",
        "n",
        "-",
        "-e",
        "",
    ] {
        assert!(!reads_as_no_commit(word.as_bytes()), "{word}");
    }
}

#[test]
fn an_am_is_stopped_only_by_the_directory_its_own_stop_keeps() {
    let directories = |word: &[u8]| {
        let family = FAMILIES.iter().find(|family| family.word == word).unwrap();
        match family.trigger {
            Trigger::Stopped(directories) => directories,
            trigger => panic!("{trigger:?}"),
        }
    };
    assert_eq!(directories(b"rebase"), ["rebase-merge", "rebase-apply"]);
    assert_eq!(directories(b"am"), ["rebase-apply"]);
}

#[test]
fn branch_reads_as_delete_every_spelling_git_reads_so() {
    for word in [
        "-d",
        "--delete",
        "--delet",
        "--dele",
        "--del",
        "--de",
        "--d",
        "-vd",
        "-dv",
        "-dr",
        "-rd",
        "-fd",
        "-qvd",
        "-uorigin/dev",
    ] {
        assert!(reads_as_delete(word.as_bytes()), "{word}");
    }
    for word in [
        "-D",
        "-Dd",
        "-dD",
        "-vD",
        "--",
        "--de=",
        "--delete=x",
        "--deleted",
        "--delete-merged",
        "--D",
        "--Delete",
        "---d",
        "d",
        "delete",
        "-",
        "-v",
        "",
    ] {
        assert!(!reads_as_delete(word.as_bytes()), "{word}");
    }
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
    let stopped = Trigger::Stopped(&["rebase-merge", "rebase-apply"]);
    for trigger in [stopped, picking] {
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
    }
    // A pick that exited 0 stopped nothing; a rebase stopped at `edit` or `break` exits 0
    // with its directory in place, so the stop is asked about whatever the end.
    assert!(!follows(picking, &End::Code(0), unasked));
    for (answer, hinted) in [
        (Answer::Yes, true),
        (Answer::No, false),
        (Answer::Unanswered, false),
    ] {
        assert_eq!(
            follows(stopped, &End::Code(0), |question| {
                assert_eq!(
                    question,
                    Question::Stopped(&["rebase-merge", "rebase-apply"])
                );
                answer
            }),
            hinted,
            "{answer:?}"
        );
    }
    assert!(follows(
        Trigger::Stopped(&["rebase-apply"]),
        &End::Code(1),
        |question| {
            assert_eq!(question, Question::Stopped(&["rebase-apply"]));
            Answer::Yes
        }
    ));
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
                "'git dupe commit --amend'",
                "'git dupe rebase --edit-todo'",
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
                "'git dupe commit --allow-empty'",
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
