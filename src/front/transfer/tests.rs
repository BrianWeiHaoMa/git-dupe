use super::*;

fn git(code: u8) -> Outcome {
    Outcome::Git(End::Code(code))
}

fn upstream(remote: &str) -> Upstream {
    upstream_of(remote.as_bytes())
}

fn upstream_of(remote: &[u8]) -> Upstream {
    Upstream {
        remote: remote.to_vec(),
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
    let without = Then::Push(Some(upstream("origin")));
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
fn the_upstream_hint_names_the_remote_as_a_shell_reads_it_back_and_head() {
    let named = |remote: &[u8]| no_upstream(&upstream_of(remote));
    for (remote, command) in [
        (&b"origin"[..], &b"'git dupe push -u origin HEAD'"[..]),
        (
            b"/srv/private.git",
            b"'git dupe push -u /srv/private.git HEAD'",
        ),
        (
            b"git@host:me/p.git",
            b"'git dupe push -u git@host:me/p.git HEAD'",
        ),
        // `push` reads a first word that is no option as the repository, `+` and all.
        (b"+backup", b"'git dupe push -u +backup HEAD'"),
        // A byte above ASCII is no shell's concern.
        (
            "caf\u{e9}".as_bytes(),
            "'git dupe push -u caf\u{e9} HEAD'".as_bytes(),
        ),
        // Quoted where a shell would read a byte otherwise.
        (b"my remote", b"'git dupe push -u \"my remote\" HEAD'"),
        (b"it's", b"'git dupe push -u \"it's\" HEAD'"),
        (b"$HOME", b"'git dupe push -u \"\\$HOME\" HEAD'"),
        // Empty, read as an option, or breaking the line: the placeholder.
        (b"", b"'git dupe push -u <remote> HEAD'"),
        (b"-x", b"'git dupe push -u <remote> HEAD'"),
        (b"a\nb", b"'git dupe push -u <remote> HEAD'"),
    ] {
        let line = named(remote);
        assert!(
            line.windows(command.len()).any(|window| window == command),
            "{}: {}",
            line.escape_ascii(),
            command.escape_ascii()
        );
    }
}

#[test]
fn each_hint_is_one_line_naming_the_git_dupe_commands_and_the_plain_ones() {
    let upstream = no_upstream(&upstream("origin"));
    for (hint, parts) in [
        (
            &upstream[..],
            &["'git dupe push -u origin HEAD'", "plain 'git push'"][..],
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
