use super::*;

fn words(words: &[&str]) -> Vec<Vec<u8>> {
    words.iter().map(|word| word.as_bytes().to_vec()).collect()
}

fn word_of(words: &[&str]) -> Option<&'static [u8]> {
    admitted(&self::words(words))
        .first()
        .map(|family| family.word)
}

/// An `ask` for a decision that must put no question.
fn unasked(question: Question) -> Answer {
    panic!("asked {question:?}")
}

/// Whether the line of an entry with `trigger` follows a run that ended as `end`.
fn holds(trigger: Trigger, end: &End, ask: impl FnMut(Question) -> Answer) -> bool {
    let family = Family {
        word: b"word",
        excluding: None,
        requiring: None,
        trigger,
        line: b"line",
    };
    follows(&family, end, ask).is_some()
}

/// The line `decided` gives for the entries `typed` admits after a run that ended as `end`,
/// each question answered as `answers` says, and the questions it put, in order.
fn decision(
    typed: &[&str],
    end: &End,
    answers: &[(Question, Answer)],
) -> (Option<&'static [u8]>, Vec<Question>) {
    let mut asked = Vec::new();
    let line = decided(&admitted(&words(typed)), end, |question| {
        asked.push(question);
        answers
            .iter()
            .find(|(answered, _)| *answered == question)
            .map(|(_, answer)| *answer)
            .unwrap_or_else(|| panic!("asked {question:?}"))
    });
    (line, asked)
}

#[test]
fn a_word_not_in_the_table_admits_no_hint() {
    for typed in [
        &["log"][..],
        &["stash", "pop"],
        &["Checkout", "HEAD~1"],
        &["mergetool"],
        &["commit-tree", "x"],
        &[],
    ] {
        assert_eq!(word_of(typed), None, "{typed:?}");
    }
}

#[test]
fn the_words_admit_or_keep_away_each_family() {
    let admitting: [&[&str]; 26] = [
        &["rebase", "side"],
        &["rebase", "--continue"],
        &["merge", "side"],
        &["merge", "--abort"],
        &["commit", "-m", "x"],
        &["commit", "--amend"],
        &["am", "patch"],
        &["cherry-pick", "side"],
        &["revert", "HEAD~1"],
        &["checkout", "HEAD~1"],
        // `-d` keeps the hint away for `switch` alone, as the table writes it, although
        // `checkout` reads it as `--detach` too; after `--` a word is a path or a revision.
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
    let kept_away: [&[&str]; 31] = [
        &["merge", "-h"],
        &["commit", "--amend", "-h"],
        // An amend Git was told may be empty refuses for another cause; with no `--amend`
        // the words do not keep a stopped pick's hint away.
        &["commit", "--amend", "--allow-empty"],
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

/// A rebase and an `am` share `rebase-apply`, each marking it with a file of its own (S12).
#[test]
fn a_rebase_and_an_am_are_each_stopped_only_by_what_their_own_stop_keeps() {
    let markers = |word: &[u8]| {
        let family = FAMILIES.iter().find(|family| family.word == word).unwrap();
        match family.trigger {
            Trigger::Stopped(markers) => markers,
            trigger => panic!("{trigger:?}"),
        }
    };
    assert_eq!(
        markers(b"rebase"),
        [
            Marker::Directory("rebase-merge"),
            Marker::Holding("rebase-apply", "rebasing")
        ]
    );
    assert_eq!(
        markers(b"am"),
        [Marker::Holding("rebase-apply", "applying")]
    );
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
        assert_eq!(!found.is_empty(), admitted, "{before:?}");
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
        assert_eq!(!found.is_empty(), admitted, "{typed:?}");
    }
}

#[test]
fn each_trigger_follows_its_end_and_state() {
    let picking = Trigger::Picking("CHERRY_PICK_HEAD");
    let stopped = Trigger::Stopped(A_REBASE_STOPPED);
    for end in [End::Code(1), End::Code(128), End::Signal(9)] {
        for (answer, hinted) in [
            (Answer::Yes, true),
            (Answer::No, false),
            (Answer::Unanswered, false),
        ] {
            assert_eq!(
                holds(stopped, &end, |_| answer),
                hinted,
                "{end:?} {answer:?}"
            );
        }
        // A pick stands where its pseudo-reference verifies and no rebase stands stopped
        // beside it: a rebase stopped at a pick that became empty keeps `CHERRY_PICK_HEAD`.
        // A stopped `am` beside it is no rebase.
        for (verifies, rebasing, hinted) in [
            (Answer::Yes, Answer::No, true),
            (Answer::Yes, Answer::Yes, false),
            (Answer::Yes, Answer::Unanswered, false),
            (Answer::No, Answer::No, false),
            (Answer::Unanswered, Answer::No, false),
        ] {
            let follow = holds(picking, &end, |question| {
                if question == Question::Verifies("CHERRY_PICK_HEAD") {
                    verifies
                } else {
                    assert_eq!(question, Question::Stopped(A_REBASE_STOPPED));
                    assert_eq!(verifies, Answer::Yes, "asked after {verifies:?}");
                    rebasing
                }
            });
            assert_eq!(follow, hinted, "{end:?} {verifies:?} {rebasing:?}");
        }
    }
    // A pick that exited 0 stopped nothing; a rebase stopped at `edit` or `break` exits 0
    // with its directory in place, so the stop is asked about whatever the end.
    assert!(!holds(picking, &End::Code(0), unasked));
    for (answer, hinted) in [
        (Answer::Yes, true),
        (Answer::No, false),
        (Answer::Unanswered, false),
    ] {
        assert_eq!(
            holds(stopped, &End::Code(0), |question| {
                assert_eq!(question, Question::Stopped(A_REBASE_STOPPED));
                answer
            }),
            hinted,
            "{answer:?}"
        );
    }
    assert!(holds(
        Trigger::Stopped(AN_AM_STOPPED),
        &End::Code(1),
        |question| {
            assert_eq!(question, Question::Stopped(AN_AM_STOPPED));
            Answer::Yes
        }
    ));
    assert!(holds(Trigger::ExitedOne, &End::Code(1), unasked));
    for end in [End::Code(0), End::Code(128), End::Signal(1), End::Signal(9)] {
        assert!(!holds(Trigger::ExitedOne, &end, unasked), "{end:?}");
    }
    // A commit during a stopped pick exits 1 (its lines: below); an autostash that came
    // back with conflicts leaves its run exiting 0.
    for (answer, hinted) in [
        (Answer::Yes, true),
        (Answer::No, false),
        (Answer::Unanswered, false),
    ] {
        let follow = holds(Trigger::Unmerged, &End::Code(0), |asked| {
            assert_eq!(asked, Question::Unmerged);
            answer
        });
        assert_eq!(follow, hinted, "{answer:?}");
    }
    let while_picking = Trigger::ExitedOneWhile("CHERRY_PICK_HEAD");
    for end in [End::Code(0), End::Code(128), End::Signal(1), End::Signal(9)] {
        assert!(!holds(while_picking, &end, unasked), "{end:?}");
    }
    for end in [
        End::Code(1),
        End::Code(128),
        End::Signal(9),
        End::Signal(15),
    ] {
        assert!(!holds(Trigger::Unmerged, &end, unasked), "{end:?}");
    }
}

#[test]
fn a_rebase_is_asked_about_its_autostash_only_where_it_exited_0_and_left_no_stop() {
    let stopped = Question::Stopped(A_REBASE_STOPPED);
    let unmerged = Question::Unmerged;
    let rebase = &["rebase", "side"][..];
    // Stopped, the stop's line decides and nothing is asked after it.
    assert_eq!(
        decision(rebase, &End::Code(0), &[(stopped, Answer::Yes)]),
        (Some(REBASE), vec![stopped])
    );
    assert_eq!(
        decision(
            rebase,
            &End::Code(0),
            &[(stopped, Answer::No), (unmerged, Answer::Yes)]
        ),
        (Some(AUTOSTASH), vec![stopped, unmerged])
    );
    for answer in [Answer::No, Answer::Unanswered] {
        assert_eq!(
            decision(
                rebase,
                &End::Code(0),
                &[(stopped, Answer::No), (unmerged, answer)]
            ),
            (None, vec![stopped, unmerged]),
            "{answer:?}"
        );
    }
    // A question that answers nothing ends the decision.
    assert_eq!(
        decision(rebase, &End::Code(0), &[(stopped, Answer::Unanswered)]),
        (None, vec![stopped])
    );
    // `--quit` leaves its conflicts unmerged with no autostash applied: not asked.
    assert_eq!(
        decision(
            &["rebase", "--quit"],
            &End::Code(0),
            &[(stopped, Answer::No)]
        ),
        (None, vec![stopped])
    );
    for end in [End::Code(1), End::Code(128), End::Signal(9)] {
        assert_eq!(
            decision(rebase, &end, &[(stopped, Answer::No)]),
            (None, vec![stopped]),
            "{end:?}"
        );
    }
}

#[test]
fn a_merge_is_asked_about_its_autostash_only_after_exit_0() {
    let merge = &["merge", "--no-edit", "side"][..];
    for (answer, line) in [
        (Answer::Yes, Some(AUTOSTASH)),
        (Answer::No, None),
        (Answer::Unanswered, None),
    ] {
        assert_eq!(
            decision(merge, &End::Code(0), &[(Question::Unmerged, answer)]),
            (line, vec![Question::Unmerged]),
            "{answer:?}"
        );
    }
    // A merge that stops on its own conflicts exits 1 with unmerged entries: no question.
    for end in [End::Code(1), End::Code(128), End::Signal(9)] {
        assert_eq!(decision(merge, &end, &[]), (None, vec![]), "{end:?}");
    }
    for typed in [&["merge", "--quit"][..], &["merge", "-q", "--quit"]] {
        assert_eq!(
            decision(typed, &End::Code(0), &[]),
            (None, vec![]),
            "{typed:?}"
        );
    }
    // An abbreviation of `--quit` is refused as ambiguous, and `--` ends the words read.
    for typed in [&["merge", "--qui"][..], &["merge", "x", "--", "--quit"]] {
        assert_eq!(
            decision(typed, &End::Code(0), &[(Question::Unmerged, Answer::Yes)]),
            (Some(AUTOSTASH), vec![Question::Unmerged]),
            "{typed:?}"
        );
    }
}

#[test]
fn a_commit_is_decided_by_its_amend_and_then_by_a_stopped_pick_and_the_stop_beside_it() {
    let pick = Question::Verifies("CHERRY_PICK_HEAD");
    let rebase = Question::Stopped(A_REBASE_STOPPED);
    // `--amend` is read in every prefix Git reads as it, `--allow-empty` exactly, each
    // before the first `--`.
    for typed in [
        &["commit", "--amend"][..],
        &["commit", "--amend", "--no-edit"],
        &["commit", "--amen", "--no-edit"],
        &["commit", "--ame"],
        &["commit", "--am"],
        &["commit", "--allow-empty-message", "--amend"],
        &["commit", "--amend", "--", "--allow-empty"],
    ] {
        assert_eq!(
            decision(typed, &End::Code(1), &[]),
            (Some(EMPTY_AMEND), vec![]),
            "{typed:?}"
        );
        for end in [End::Code(0), End::Code(128), End::Signal(9)] {
            assert_eq!(
                decision(typed, &end, &[]),
                (None, vec![]),
                "{typed:?} {end:?}"
            );
        }
    }
    for typed in [
        &["commit", "--amend", "--allow-empty"][..],
        &["commit", "--allow-empty", "-m", "x", "--amend"],
    ] {
        assert_eq!(
            decision(typed, &End::Code(1), &[]),
            (None, vec![]),
            "{typed:?}"
        );
    }
    // Without `--amend`, a stopped cherry-pick is asked about, and a stopped revert never:
    // Git's advice for a commit during one names only the revert's own commands. Where the
    // pick stands, so may a rebase stopped at a pick that became empty, whose line follows
    // instead; a stopped `am` beside the pick is no rebase, and the pick's line follows.
    for typed in [
        &["commit", "-m", "x"][..],
        &["commit"],
        &["commit", "--allow-empty"],
        &["commit", "--a"],
        &["commit", "--amended"],
        &["commit", "--", "--amend"],
    ] {
        assert_eq!(
            decision(
                typed,
                &End::Code(1),
                &[(pick, Answer::Yes), (rebase, Answer::No)]
            ),
            (Some(CHERRY_PICK), vec![pick, rebase]),
            "{typed:?}"
        );
        assert_eq!(
            decision(
                typed,
                &End::Code(1),
                &[(pick, Answer::Yes), (rebase, Answer::Yes)]
            ),
            (Some(REBASE), vec![pick, rebase]),
            "{typed:?}"
        );
        assert_eq!(
            decision(
                typed,
                &End::Code(1),
                &[(pick, Answer::Yes), (rebase, Answer::Unanswered)]
            ),
            (None, vec![pick, rebase]),
            "{typed:?}"
        );
        for answer in [Answer::No, Answer::Unanswered] {
            assert_eq!(
                decision(typed, &End::Code(1), &[(pick, answer)]),
                (None, vec![pick]),
                "{typed:?} {answer:?}"
            );
        }
    }
    let typed = &["commit", "-m", "x"][..];
    for end in [End::Code(0), End::Code(128), End::Signal(1), End::Signal(9)] {
        assert_eq!(decision(typed, &end, &[]), (None, vec![]), "{end:?}");
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
            holds(Trigger::Detached, &End::Code(0), |question| {
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
        assert!(!holds(Trigger::Detached, &end, unasked), "{end:?}");
    }
}

#[test]
fn each_hint_names_the_git_dupe_commands_and_the_plain_ones_project() {
    let named: [(&[u8], &[&str]); 9] = [
        (
            REBASE,
            &[
                "'git dupe add <path>'",
                "'git dupe commit --amend'",
                "'git dupe commit --allow-empty'",
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
        (
            AUTOSTASH,
            &[
                "'git dupe add <path>'",
                "'git dupe stash drop'",
                "'git dupe reset --hard'",
                "'git dupe stash pop'",
            ],
        ),
        (
            MERGED_CHANGES,
            &[
                "'git dupe add <path>'",
                "'git dupe stash drop'",
                "'git dupe reset --hard'",
                "'git dupe stash pop'",
            ],
        ),
        (
            EMPTY_AMEND,
            &[
                "the same 'git dupe commit' as typed with --allow-empty added",
                "'git dupe reset HEAD^'",
            ],
        ),
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

#[test]
fn a_word_git_reads_as_merge_admits_the_entry_of_its_command_after_the_detached_one() {
    let unmerged = [
        (Question::OnABranch, Answer::Yes),
        (Question::Unmerged, Answer::Yes),
    ];
    for typed in [
        &["switch", "-m", "side"][..],
        &["switch", "--merge", "side"],
        &["checkout", "-m", "side"],
        &["checkout", "--merge", "side"],
        &["checkout", "-qm", "side"],
        &["switch", "-m", "-q", "side"],
    ] {
        let (line, asked) = decision(typed, &End::Code(0), &unmerged);
        assert_eq!(line, Some(MERGED_CHANGES), "{typed:?}");
        assert_eq!(asked.last(), Some(&Question::Unmerged), "{typed:?}");
        // Refused: nothing is asked of the index.
        assert_eq!(decision(typed, &End::Code(1), &unmerged).0, None);
        let clean = [
            (Question::OnABranch, Answer::Yes),
            (Question::Unmerged, Answer::No),
        ];
        assert_eq!(decision(typed, &End::Code(0), &clean).0, None, "{typed:?}");
    }
    for typed in [
        &["switch", "side"][..],
        &["switch", "--", "-m"],
        &["switch", "--merge=x"],
        &["switch", "--me"],
        &["switch", "-m", "side", "-h"],
        &["checkout", "--mer"],
    ] {
        let admitted = admitted(&words(typed));
        assert!(
            !admitted.iter().any(|family| family.line == MERGED_CHANGES),
            "{typed:?}"
        );
    }
    // A detached `HEAD` decides first, in the table's order.
    let left = [(Question::OnABranch, Answer::No)];
    let (line, _) = decision(&["switch", "-m", "v1"], &End::Code(0), &left);
    assert_eq!(line, Some(DETACHED));
}

#[test]
fn only_a_prefix_git_reads_as_amend_is_one() {
    for word in ["--amend", "--amen", "--ame", "--am"] {
        assert!(reads_as_amend(word.as_bytes()), "{word}");
    }
    for word in ["--a", "--", "-a", "--amended", "--all", "--amend=x"] {
        assert!(!reads_as_amend(word.as_bytes()), "{word}");
    }
}

#[test]
fn the_merged_changes_line_says_nothing_of_a_stash_entry_unconditioned() {
    let line = str::from_utf8(MERGED_CHANGES).unwrap();
    let conditioned = line.find("where Git also kept them").unwrap();
    for later in [
        "'git dupe stash drop'",
        "'git dupe reset --hard'",
        "'git dupe stash pop'",
    ] {
        assert!(line.find(later).unwrap() > conditioned, "{later}");
    }
    assert!(line.find("'git dupe add <path>'").unwrap() < conditioned);
}
