use super::*;

fn git(code: u8) -> Outcome {
    Outcome::Git(End::Code(code))
}

fn upstream(remote: &str, branch: &str) -> Upstream {
    Upstream {
        remote: remote.as_bytes().to_vec(),
        branch: branch.as_bytes().to_vec(),
    }
}

#[test]
fn the_no_remote_hint_follows_a_run_that_did_not_exit_0() {
    let then = Then::NoRemoteChosen;
    assert_eq!(hint(&then, &git(0)), None);
    for ended in [git(128), git(1), Outcome::Git(End::Signal(9))] {
        assert_eq!(hint(&then, &ended).as_deref(), Some(NO_REMOTE_CHOSEN));
    }
    // A run that did not start, or a refusal, is no run.
    assert_eq!(hint(&then, &Outcome::Refused), None);
    for ended in [git(0), git(1), git(128), Outcome::Git(End::Signal(9))] {
        assert_eq!(hint(&Then::Nothing, &ended), None);
    }
}

#[test]
fn a_push_hints_at_the_upstream_after_128_and_at_a_rejection_after_1() {
    let without = Then::Push(Some(upstream("origin", "main")));
    let upstream_hint = hint(&without, &git(128)).unwrap();
    assert!(
        upstream_hint.starts_with(b"the current private branch has no upstream"),
        "{}",
        upstream_hint.escape_ascii()
    );
    for then in [&without, &Then::Push(None)] {
        assert_eq!(hint(then, &git(1)).as_deref(), Some(REJECTED));
        for ended in [
            git(0),
            git(129),
            Outcome::Git(End::Signal(1)),
            Outcome::Refused,
        ] {
            assert_eq!(hint(then, &ended), None, "{then:?}");
        }
    }
    // Given a repository, or detached, a push owes no upstream hint after 128.
    assert_eq!(hint(&Then::Push(None), &git(128)), None);
    // The no-remote hint and the rejected one are never both owed.
    assert_eq!(
        hint(&Then::NoRemoteChosen, &git(1)).as_deref(),
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
            assert_eq!(hint(&then, &ended).as_deref(), Some(NOT_TRACKING));
        }
        assert_eq!(hint(&then, &git(0)), None);
    }
    assert_eq!(
        hint(&pull(false, true), &git(128)).as_deref(),
        Some(UNRECONCILED)
    );
    for ended in [
        git(0),
        git(1),
        Outcome::Git(End::Signal(128)),
        Outcome::Refused,
    ] {
        assert_eq!(hint(&pull(false, true), &ended), None);
    }
    assert_eq!(hint(&pull(false, false), &git(128)), None);
}

#[test]
fn the_upstream_hint_names_the_remote_and_branch_a_shell_takes_as_written() {
    let named = |remote: &str, branch: &str| {
        String::from_utf8(no_upstream(&upstream(remote, branch))).unwrap()
    };
    for (remote, branch, command) in [
        ("origin", "main", "'git dupe push -u origin main'"),
        (
            "/srv/private.git",
            "topic/x",
            "'git dupe push -u /srv/private.git topic/x'",
        ),
        (
            "git@host:me/p.git",
            "v1.2",
            "'git dupe push -u git@host:me/p.git v1.2'",
        ),
        // Empty, read as an option, or holding a byte a shell reads otherwise.
        ("", "main", "'git dupe push -u <remote> main'"),
        ("-x", "main", "'git dupe push -u <remote> main'"),
        ("my remote", "main", "'git dupe push -u <remote> main'"),
        ("it's", "main", "'git dupe push -u <remote> main'"),
        ("~/private", "main", "'git dupe push -u <remote> main'"),
        ("origin", "a;b", "'git dupe push -u origin <branch>'"),
        // A branch `+topic`, which Git would read as forcing a push of `topic`.
        ("origin", "+topic", "'git dupe push -u origin <branch>'"),
        ("origin", "$HOME", "'git dupe push -u origin <branch>'"),
    ] {
        let line = named(remote, branch);
        assert!(line.contains(command), "{line}: {command}");
    }
}

#[test]
fn each_hint_is_one_line_naming_the_git_dupe_commands_and_the_plain_ones() {
    let upstream = no_upstream(&upstream("origin", "main"));
    for (hint, parts) in [
        (
            &upstream[..],
            &["'git dupe push -u origin main'", "plain 'git push'"][..],
        ),
        (
            REJECTED,
            &["'git dupe pull'", "'git dupe fetch'", "plain 'git pull'"],
        ),
        (
            NOT_TRACKING,
            &[
                "'git dupe pull <remote> <branch>'",
                "'git dupe branch --set-upstream-to=<remote>/<branch>'",
                "plain 'git pull' and 'git branch'",
            ],
        ),
        (
            UNRECONCILED,
            &[
                "'git dupe config pull.rebase false'",
                "'git dupe config pull.rebase true'",
                "'git dupe config pull.ff only'",
                "plain 'git config'",
            ],
        ),
    ] {
        let hint = String::from_utf8(hint.to_vec()).unwrap();
        for part in parts {
            assert!(hint.contains(part), "{hint}: {part}");
        }
        assert!(hint.contains("the project's repository"), "{hint}");
        assert!(!hint.contains('\n'), "{hint}");
    }
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

#[test]
fn the_hint_names_the_git_dupe_commands_and_never_says_no_remote_exists() {
    let hint = String::from_utf8(NO_REMOTE_CHOSEN.to_vec()).unwrap();
    assert!(
        hint.contains("'git dupe remote add <name> <url>'"),
        "{hint}"
    );
    assert!(
        hint.contains("'git dupe push -u <name> <branch>'"),
        "{hint}"
    );
    assert!(
        hint.find("git dupe remote add") < hint.find("git dupe push -u"),
        "{hint}"
    );
    assert!(!hint.contains("no remote"), "{hint}");
    assert!(!hint.contains('\n'), "{hint}");
}
