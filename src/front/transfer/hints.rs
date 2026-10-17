//! The lines of the hints after a `push` or `pull` the guard let through (`Holds/G18`,
//! G25): each one `hint:` line naming the `git dupe` commands and saying that the plain
//! ones act on the project's repository. Which line follows which run is `transfer`'s.
//!
//! What the code below cannot show:
//!
//! - A remote or a branch a line names as part of a command is written as a shell reads
//!   it back as one word (`shell_word`), or as a placeholder where none is to be named, so
//!   that the command, typed as the line shows it, runs as the line says and the line
//!   stays one line (F8, G25).

use crate::guards::quoted::shell_word;

/// The hint after a failed `push` whose configuration chooses no remote: true whichever
/// remote Git took, the only one of another name included, so it never says that none
/// exists.
pub const NO_REMOTE_CHOSEN: &[u8] = b"to push private history, 'git dupe remote add <name> <url>' \
    adds a private remote and 'git dupe push -u <name> <branch>' pushes to it; plain \
    'git remote' and 'git push' act on the project's repository";

/// The hint after a rejected `push` given no repository: Git's advice for a rejected
/// update names plain `git pull`. Its condition names the upstream, which is what `pull`
/// and `fetch` given no repository reach: a hook's refusal, or a push to a remote other
/// than the upstream's or before the branch has one, leaves the condition false and the
/// line true; given a repository, `rejected` names it.
pub const REJECTED: &[u8] = b"where a private push is rejected because the branch's upstream \
    holds commits the branch lacks, 'git dupe pull', or 'git dupe fetch' and then a merge or \
    rebase, brings them here before the next push; plain 'git pull' and 'git fetch' act on \
    the project's repository";

/// The hint after a failed `pull` on a branch with no upstream.
pub const NOT_TRACKING: &[u8] = b"the current private branch has no upstream to pull from: \
    'git dupe pull <remote> <branch>' pulls a branch this once, and \
    'git dupe branch --set-upstream-to=<remote>/<branch>' makes it the upstream; plain \
    'git pull' and 'git branch' act on the project's repository";

/// The hint after a `pull` that exited 128 with no way to reconcile chosen.
pub const UNRECONCILED: &[u8] = b"where a private pull finds divergent branches, \
    'git dupe config pull.rebase false' has it merge, 'git dupe config pull.rebase true' \
    rebase, and 'git dupe config pull.ff only' take only a fast-forward; plain \
    'git config' acts on the project's repository";

/// The hint after a `push` that exited 128 on a branch with no upstream, naming the
/// remote Git takes and `HEAD` for the branch, whose own name a tag or another ref may
/// share.
pub fn no_upstream(remote: &[u8]) -> Vec<u8> {
    [
        &b"the current private branch has no upstream: 'git dupe push -u "[..],
        &offered_remote(remote),
        b" HEAD' pushes it and sets one; plain 'git push' acts on the project's repository",
    ]
    .concat()
}

/// The hint after a `push` that exited 128 on a branch with no commit and no upstream:
/// the push of `no_upstream` once a commit gives it something to push, which `commit`
/// makes only of something staged.
pub fn unborn(remote: &[u8]) -> Vec<u8> {
    [
        &b"the current private branch has no commit yet and no upstream: once something is \
            staged, 'git dupe commit' makes its first commit, and then 'git dupe push -u "[..],
        &offered_remote(remote),
        b" HEAD' pushes it and sets one; plain 'git commit' and 'git push' act on the \
            project's repository",
    ]
    .concat()
}

/// The hint after a `push` that exited 128 on a branch whose upstream has another name,
/// naming the upstream by the raw value of `merge`, `refs/heads/` kept: the short name
/// Git's advice writes is ambiguous where the remote has a tag of that name. Neither push
/// has a source on a branch with no commit, which the line's condition leaves out.
pub fn other_name(remote: &[u8], merge: &[u8]) -> Vec<u8> {
    let remote = offered_remote(remote);
    let to_the_upstream = if merge.is_empty() || merge.iter().any(u8::is_ascii_control) {
        b"HEAD:<upstream>".to_vec()
    } else {
        shell_word(&[&b"HEAD:"[..], merge].concat())
    };
    [
        &b"the upstream of the current private branch has another name than the branch: \
            where the branch has a commit, 'git dupe push "[..],
        &remote,
        b" ",
        &to_the_upstream,
        b"' pushes it to that upstream, and 'git dupe push ",
        &remote,
        b" HEAD' to a branch of the branch's own name there; plain 'git push' acts on the \
            project's repository",
    ]
    .concat()
}

/// The hint after a `push` that exited 128 on a detached `HEAD`. It names a branch the
/// remote has, as Git's advice does: given a commit and no ref, Git cannot tell that a
/// new name is to be a branch, and refuses it.
pub fn detached(remote: &[u8]) -> Vec<u8> {
    [
        &b"the private HEAD is detached, on no branch: 'git dupe push "[..],
        &offered_remote(remote),
        b" HEAD:<branch>' pushes the history leading to it to the branch <branch> the \
            remote has; plain 'git push' acts on the project's repository",
    ]
    .concat()
}

/// The hint after a rejected `push`, naming the repository word `given`, the destination
/// that rejected it, and `REJECTED` given none. Its condition holds where `fetch` of the
/// word reaches that one destination: not for a remote with a push URL alone or one other
/// than its fetch URL, nor for a remote group, whose `pull` refuses a refspec.
pub fn rejected(given: Option<&[u8]>) -> Vec<u8> {
    let Some(given) = given else {
        return REJECTED.to_vec();
    };
    let remote = offered_remote(given);
    [
        &b"where a private push to "[..],
        &remote,
        b" is rejected because the remote's <branch> holds commits the push lacks, and \
            'git dupe fetch ",
        &remote,
        b"' fetches from that remote alone, 'git dupe pull ",
        &remote,
        b" <branch>', or 'git dupe fetch ",
        &remote,
        b"' and then a merge or rebase, brings them here before the next push; plain \
            'git pull' and 'git fetch' act on the project's repository",
    ]
    .concat()
}

/// The remote's name or URL `name` as a shell reads it back as one word, or the
/// placeholder `<remote>` where none is to be named: a value from the configuration may
/// be empty, begin with `-`, which `push` reads as an option, or hold a control byte,
/// which would break the line (F8).
fn offered_remote(name: &[u8]) -> Vec<u8> {
    let named =
        name.first().is_some_and(|&first| first != b'-') && !name.iter().any(u8::is_ascii_control);
    if named {
        shell_word(name)
    } else {
        b"<remote>".to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_upstream_hint_names_the_remote_as_a_shell_reads_it_back_and_head() {
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
            for line in [no_upstream(remote), unborn(remote)] {
                assert!(
                    holds(&line, command),
                    "{}: {}",
                    line.escape_ascii(),
                    command.escape_ascii()
                );
            }
            // Every hint that names a remote names it so.
            let offered = &command[b"'git dupe push -u ".len()..command.len() - b" HEAD'".len()];
            let named_in = |line: Vec<u8>, before: &[u8], after: &[u8]| {
                let expected = [before, offered, after].concat();
                assert!(
                    holds(&line, &expected),
                    "{}: {}",
                    line.escape_ascii(),
                    expected.escape_ascii()
                );
            };
            named_in(detached(remote), b"'git dupe push ", b" HEAD:<branch>'");
            named_in(
                other_name(remote, b"refs/heads/main"),
                b"'git dupe push ",
                b" HEAD:refs/heads/main'",
            );
            named_in(
                other_name(remote, b"refs/heads/main"),
                b"'git dupe push ",
                b" HEAD'",
            );
            named_in(rejected(Some(remote)), b"'git dupe pull ", b" <branch>'");
            named_in(rejected(Some(remote)), b"'git dupe fetch ", b"'");
        }
    }

    fn holds(line: &[u8], part: &[u8]) -> bool {
        line.windows(part.len()).any(|window| window == part)
    }

    /// The upstream is the raw value of `merge`, `refs/heads/` kept, because the short name
    /// Git's advice writes is ambiguous beside a tag of that name on the remote.
    #[test]
    fn the_other_name_hint_writes_the_upstream_by_its_raw_value_as_a_shell_reads_it_back() {
        for (merge, command) in [
            (
                &b"refs/heads/main"[..],
                &b"'git dupe push origin HEAD:refs/heads/main'"[..],
            ),
            // Without the prefix, the value as Git's advice writes it.
            (b"topic", b"'git dupe push origin HEAD:topic'"),
            (b"refs/tags/v1", b"'git dupe push origin HEAD:refs/tags/v1'"),
            (
                b"refs/heads/feature/x",
                b"'git dupe push origin HEAD:refs/heads/feature/x'",
            ),
            (
                b"refs/heads/it's",
                b"'git dupe push origin \"HEAD:refs/heads/it's\"'",
            ),
            // Nothing to name, or a byte that would break the line: the placeholder.
            (b"", b"'git dupe push origin HEAD:<upstream>'"),
            (
                b"refs/heads/a\nb",
                b"'git dupe push origin HEAD:<upstream>'",
            ),
        ] {
            let line = other_name(b"origin", merge);
            assert!(
                holds(&line, command),
                "{}: {}",
                line.escape_ascii(),
                command.escape_ascii()
            );
            assert!(holds(&line, b"'git dupe push origin HEAD'"));
        }
    }

    #[test]
    fn each_hint_is_one_line_naming_the_git_dupe_commands_and_the_plain_ones() {
        let upstream = no_upstream(b"origin");
        let unborn = unborn(b"origin");
        let other_name = other_name(b"origin", b"refs/heads/main");
        let detached = detached(b"origin");
        let rejected_by = rejected(Some(b"origin"));
        for (hint, parts) in [
            (
                &upstream[..],
                &["'git dupe push -u origin HEAD'", "plain 'git push'"][..],
            ),
            (
                &unborn,
                &[
                    "'git dupe commit'",
                    "'git dupe push -u origin HEAD'",
                    "plain 'git commit' and 'git push'",
                ],
            ),
            (
                &other_name,
                &[
                    "'git dupe push origin HEAD:refs/heads/main'",
                    "'git dupe push origin HEAD'",
                    "plain 'git push'",
                ],
            ),
            (
                &detached,
                &["'git dupe push origin HEAD:<branch>'", "plain 'git push'"],
            ),
            (
                REJECTED,
                &["'git dupe pull'", "'git dupe fetch'", "plain 'git pull'"],
            ),
            (
                &rejected_by,
                &[
                    "'git dupe pull origin <branch>'",
                    "'git dupe fetch origin'",
                    "plain 'git pull' and 'git fetch'",
                ],
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
            assert!(!hint.contains("  "), "{hint}");
        }
        // The commit comes first: the push alone has no source to push on an unborn branch.
        let unborn = String::from_utf8(unborn).unwrap();
        assert!(
            unborn.find("'git dupe commit'") < unborn.find("'git dupe push -u"),
            "{unborn}"
        );
    }

    /// A line whose commands do what it says in only part of the state that draws it says
    /// in which part, before it names them.
    #[test]
    fn a_line_says_where_its_commands_do_what_it_says() {
        for (line, condition, command) in [
            (
                unborn(b"origin"),
                &b"once something is staged"[..],
                &b"'git dupe commit'"[..],
            ),
            (
                other_name(b"origin", b"refs/heads/main"),
                b"where the branch has a commit",
                b"'git dupe push origin HEAD:refs/heads/main'",
            ),
            (
                REJECTED.to_vec(),
                b"because the branch's upstream holds commits",
                b"'git dupe pull'",
            ),
            (
                rejected(Some(b"origin")),
                b"and 'git dupe fetch origin' fetches from that remote alone",
                b"'git dupe pull origin <branch>'",
            ),
        ] {
            let at = |part: &[u8]| line.windows(part.len()).position(|window| window == part);
            assert!(
                at(condition).is_some() && at(condition) < at(command),
                "{}",
                line.escape_ascii()
            );
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
}
