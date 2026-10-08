//! The exposure question: whether public Git ignores each path settle and `detach`'s
//! removal ask about, from one public `git check-ignore --no-index -v -n -z --stdin` from
//! the root, and the warnings of its answer, which `warnings` alone words; and the same
//! question about the ancestors of the paths `clean` spares under `-X` (`Holds/G16`).
//! Settle also asks it about the foreign paths, and both settle and `detach` about paths
//! another worktree's region hides (`foreign`, G3, G27).
//!
//! The question and its records are written and read by `runner::records`, which says
//! whether a record reports its path ignored; what a path that is not ignored means is
//! the keeper's. A path the caller's public listing names is not ignored whatever the
//! question answers, because Git applies no ignore rule to a path its index tracks (G7,
//! `Holds/G6, G8, G9`): `--no-index` reads the rules as if nothing were tracked. Exit 1
//! means no path matched, which is an answer; any other failure answers nothing, and a
//! partial output is never read as an answer: a path is named as still hidden by another
//! worktree only on an answer that says public Git ignores it, never on a question that
//! failed. A failure is one warning; a run killed by a signal is also handed back,
//! because `detach` ends with it (`Composition/Front`, "Lines"), while settle keeps the
//! command's status (`Composition/Keeper`). A path with a symbolic link among its
//! ancestors makes Git fail the whole run, so it is never fed: it is named as lying
//! beyond one instead.

use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::Failed;
use super::foreign::{self, Owner};
use crate::guards::operand;
use crate::guards::quoted::{quoted, shell_word};
use crate::runner::locate::Workspace;
use crate::runner::{End, Failure, Run, records};

/// What public Git says of one path that is not ignored.
#[derive(Debug, PartialEq, Eq)]
pub struct Exposed {
    /// The source and line of the rule that re-includes it, when a rule decides.
    pub rule: Option<(Vec<u8>, Vec<u8>)>,
}

/// What a path asked about is to the command asking, which decides what its warning says
/// when public Git does not ignore it, and, for a path no longer hidden here, the other
/// worktrees whose regions hide it, which its warning names when public Git ignores it
/// and does not track it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asked<'o> {
    /// A hidden path, which the region ignores unless a rule re-includes it (G6).
    Hidden,
    /// A path the region hid when the command began that is no longer hidden (G9), and
    /// whether `git dupe hide` would hide it again, which its warning then names (G25).
    Released {
        hideable: bool,
        owners: Vec<&'o Owner>,
    },
    /// A hidden path whose region `detach` deleted (G3).
    Left { owners: Vec<&'o Owner> },
    /// A hidden path both indexes track: public Git does not ignore it, and settle's own
    /// warning of G8 is the one that names it.
    TrackedByBoth,
    /// A foreign path: named only while public Git ignores it and does not track it, and
    /// never as one it can see, because it is not hidden here (G27).
    Foreign {
        hideable: bool,
        owners: Vec<&'o Owner>,
    },
}

/// What the question said of one path.
enum Answer<'e> {
    Ignored,
    Exposed(&'e Exposed),
    /// The question failed, and said nothing of it.
    Unanswered,
}

/// Why public Git does not ignore a path.
enum Seen<'e> {
    /// Its index tracks the path.
    Tracked,
    /// The question says so.
    Exposed(&'e Exposed),
}

/// The warnings of one exposure question, the count of a list that did not fit on one
/// command line (G23), and the question's run where a signal killed it, its warning among
/// the others.
pub struct Answered {
    pub warnings: Vec<Vec<u8>>,
    /// How many of the warnings name a path no longer hidden as visible to public Git: one
    /// `detach` left (G3), or one released (G9). A path beyond a symbolic link, or one the
    /// question could not answer for, is not among them.
    pub visible: usize,
    pub refused: Option<usize>,
    pub killed: Option<Failed>,
}

/// Asks public Git about each path in `paths` but those beyond a symbolic link, and
/// returns, in order, one warning per path beyond a link, the one warning that the
/// question failed, unless its list did not fit, then one per path public Git does not
/// ignore, worded by what the path is to the asker, or that it ignores, does not track,
/// and another worktree's region hides; how many of those name a path as now visible; and
/// the run where a signal killed the question. `tracked`, the paths the asker's public
/// listing named, are not ignored whatever the question answers, and are named so even
/// when it failed. `known` holds the asker's answers for ancestors already looked at.
pub fn warnings(
    workspace: &Workspace,
    paths: &[(&[u8], Asked)],
    tracked: &BTreeSet<Vec<u8>>,
    known: &mut HashMap<Vec<u8>, bool>,
) -> Answered {
    let mut warnings = Vec::new();
    let mut asked = Vec::new();
    for (path, kind) in paths {
        let path = *path;
        if beyond_a_link(workspace.root(), path, known) {
            warnings.push(
                [
                    path,
                    b" lies beyond a symbolic link, where public Git does not look; \
                      whether it is ignored was not asked",
                ]
                .concat(),
            );
        } else {
            asked.push((path, kind));
        }
    }
    let question: Vec<&[u8]> = asked.iter().map(|(path, _)| *path).collect();
    let mut refused = None;
    let mut killed = None;
    let answers = ask(workspace, &question).map_err(|failed| match failed {
        Failed::TooLong(count) => refused = Some(count),
        failed => {
            warnings.push(
                [
                    b"cannot ask public Git whether it ignores the hidden paths (",
                    &failed.cause()[..],
                    b"); exposure was not checked",
                ]
                .concat(),
            );
            if let Failed::Exited(End::Signal(_)) = failed {
                killed = Some(failed);
            }
        }
    });
    let mut visible = 0;
    for (at, (path, kind)) in asked.iter().enumerate() {
        // Unanswered: what the listing says alone stands.
        let answer = match &answers {
            Ok(answers) => match &answers[at] {
                None => Answer::Ignored,
                Some(exposed) => Answer::Exposed(exposed),
            },
            Err(()) => Answer::Unanswered,
        };
        let seen = if tracked.contains(*path) {
            Some(Seen::Tracked)
        } else if let Answer::Exposed(exposed) = answer {
            Some(Seen::Exposed(exposed))
        } else {
            None
        };
        if let Some(line) = seen.and_then(|seen| exposed_line(path, &seen, kind)) {
            if matches!(kind, Asked::Left { .. } | Asked::Released { .. }) {
                visible += 1;
            }
            warnings.push(line);
        } else if matches!(answer, Answer::Ignored) && !tracked.contains(*path) {
            warnings.extend(still_hidden_line(path, kind));
        }
    }
    Answered {
        warnings,
        visible,
        refused,
        killed,
    }
}

/// The warning for a path public Git ignores and does not track, where another
/// worktree's region hides it and this worktree no longer does or never did.
fn still_hidden_line(path: &[u8], kind: &Asked) -> Option<Vec<u8>> {
    let (owners, hideable) = match kind {
        Asked::Released { hideable, owners } | Asked::Foreign { hideable, owners } => {
            (owners, *hideable)
        }
        Asked::Left { owners } => (owners, false),
        Asked::Hidden | Asked::TrackedByBoth => return None,
    };
    (!owners.is_empty()).then(|| foreign::still_hidden(path, owners, hideable))
}

fn exposed_line(path: &[u8], seen: &Seen, kind: &Asked) -> Option<Vec<u8>> {
    let line = match kind {
        Asked::TrackedByBoth | Asked::Foreign { .. } => return None,
        Asked::Released {
            hideable: false, ..
        } => [path, b" is no longer hidden and is visible to public Git"].concat(),
        // The path is root-relative, and so is the command, as `hide`'s own hint says of
        // `unhide`; `--` keeps a path beginning with `-` a path.
        Asked::Released { hideable: true, .. } => [
            path,
            b" is no longer hidden and is visible to public Git; run from the root, \
              'git dupe hide -- ",
            &shell_word(path),
            b"' hides it again",
        ]
        .concat(),
        Asked::Left { .. } => [path, b" was hidden and is now visible to public Git"].concat(),
        Asked::Hidden => match seen {
            Seen::Tracked => [
                path,
                b" is hidden but public Git tracks it and does not ignore it",
            ]
            .concat(),
            Seen::Exposed(Exposed {
                rule: Some((source, line)),
            }) => [
                path,
                b" is hidden but public Git does not ignore it: ",
                &one_line(source),
                b":",
                line,
                b" re-includes it",
            ]
            .concat(),
            Seen::Exposed(Exposed { rule: None }) => {
                [path, b" is hidden but public Git does not ignore it"].concat()
            }
        },
    };
    Some(line)
}

/// The file of a rule, as a line names it: as Git gave it, or quoted as Git quotes it when
/// it holds a newline, which E4 keeps out of the workspace but not out of a file Git reads
/// rules from, such as a global excludes file (F8).
fn one_line(source: &[u8]) -> Vec<u8> {
    if source.contains(&b'\n') {
        quoted(source)
    } else {
        source.to_vec()
    }
}

/// Asks about every path in `paths`, in order, and returns, in the same order, `None`
/// for an ignored path and what public Git said of the others.
pub fn ask(workspace: &Workspace, paths: &[&[u8]]) -> Result<Vec<Option<Exposed>>, Failed> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let run = Run::public(["check-ignore", "--no-index", "-v", "-n", "-z", "--stdin"])
        .from(workspace.root())
        .own_paths()
        .feed(records::ignore_question(paths))
        .capture_output()
        .start()
        .map_err(|failure| match failure {
            Failure::TooLong => Failed::TooLong(paths.len()),
            failure => Failed::NotStarted(failure),
        })?;
    if run.end != End::Code(0) && run.end != End::Code(1) {
        return Err(Failed::Exited(run.end));
    }
    // Records that do not answer exactly the paths fed are no answer either.
    records(&run.stdout, paths).ok_or(Failed::Unreadable)
}

/// The paths among `paths` that public Git ignores, asked as `ask` asks. A run that
/// fails answers nothing, and the caller ends with nothing deleted.
pub fn ignored(workspace: &Workspace, paths: &[&[u8]]) -> Result<BTreeSet<Vec<u8>>, Failed> {
    let answers = ask(workspace, paths)?;
    Ok(paths
        .iter()
        .zip(answers)
        .filter(|(_, exposed)| exposed.is_none())
        .map(|(path, _)| path.to_vec())
        .collect())
}

/// What the records say of each path: `None` for an ignored one, else what exposes it.
fn records(output: &[u8], paths: &[&[u8]]) -> Option<Vec<Option<Exposed>>> {
    let records = records::ignore_records(output, paths)?;
    let exposed = records.into_iter().map(|record| {
        (!record.ignored()).then(|| Exposed {
            rule: (!record.source.is_empty())
                .then(|| (record.source.to_vec(), record.line.to_vec())),
        })
    });
    Some(exposed.collect())
}

/// Whether a directory between the root and `path` is a symbolic link, by `lstat`. The
/// answers for ancestors already looked at are kept in `known`.
pub fn beyond_a_link(root: &Path, path: &[u8], known: &mut HashMap<Vec<u8>, bool>) -> bool {
    link_above(root, path, known).is_some()
}

/// The outermost directory between the root and `path` that is a symbolic link, by
/// `lstat`, when one is: `clean` names it (G16). The answers for ancestors already looked
/// at are kept in `known`.
pub fn link_above<'p>(
    root: &Path,
    path: &'p [u8],
    known: &mut HashMap<Vec<u8>, bool>,
) -> Option<&'p [u8]> {
    operand::ancestors(path).find(|ancestor| {
        *known.entry(ancestor.to_vec()).or_insert_with(|| {
            fs::symlink_metadata(root.join(OsStr::from_bytes(ancestor)))
                .is_ok_and(|found| found.file_type().is_symlink())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_per_path_says_whether_it_is_ignored_and_which_rule_decides() {
        let output = b".git/info/exclude\x001\x00/notes\x00./notes\x00\
                       .gitignore\x003\x00!conf\x00./conf\x00\
                       \x00\x00\x00./loose\x00";
        let found = records(output, &[b"notes", b"conf", b"loose"]).expect("records");
        assert_eq!(
            found,
            [
                None,
                Some(Exposed {
                    rule: Some((b".gitignore".to_vec(), b"3".to_vec()))
                }),
                Some(Exposed { rule: None }),
            ]
        );
    }

    #[test]
    fn a_released_path_names_hide_only_where_hide_would_take_it() {
        let seen = Seen::Exposed(&Exposed { rule: None });
        let line = |hideable| {
            let released = Asked::Released {
                hideable,
                owners: Vec::new(),
            };
            exposed_line(b"-x", &seen, &released)
        };
        assert_eq!(
            line(false).unwrap(),
            b"-x is no longer hidden and is visible to public Git"
        );
        assert_eq!(
            line(true).unwrap(),
            b"-x is no longer hidden and is visible to public Git; run from the root, \
              'git dupe hide -- -x' hides it again"
        );
    }

    #[test]
    fn records_that_do_not_answer_the_paths_fed_are_no_answer() {
        let one = &b"\x00\x00\x00./a\x00"[..];
        assert!(records(one, &[b"a"]).is_some());
        // Too few, too many, another path, and a record cut short.
        assert!(records(one, &[b"a", b"b"]).is_none());
        assert!(records(one, &[]).is_none());
        assert!(records(one, &[b"b"]).is_none());
        assert!(records(b"\x00\x00\x00./a", &[b"a"]).is_none());
        assert!(records(b"", &[b"a"]).is_none());
    }

    #[test]
    fn a_path_is_echoed_byte_for_byte() {
        let output = b".git/info/exclude\x002\x00/cr[\r]x\x00./cr\rx\x00";
        assert_eq!(records(output, &[b"cr\rx"]), Some(vec![None]));
    }

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_link_at_any_depth_among_the_ancestors_is_found() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-exposure-{}", std::process::id())));
        let _ = fs::remove_dir_all(&scratch.0);
        let root = scratch.0.join("root");
        fs::create_dir_all(root.join("a/other")).unwrap();
        fs::create_dir_all(scratch.0.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(scratch.0.join("elsewhere"), root.join("a/link")).unwrap();

        assert!(beyond_a_link(&root, b"a/link/secret", &mut HashMap::new()));
        assert!(!beyond_a_link(&root, b"a/other/file", &mut HashMap::new()));
        // The link itself is a path with no link above it.
        assert!(!beyond_a_link(&root, b"a/link", &mut HashMap::new()));
        // The answers kept for ancestors already looked at are the same answers.
        let mut known = HashMap::new();
        assert!(!beyond_a_link(&root, b"a/other/file", &mut known));
        assert!(beyond_a_link(&root, b"a/link/deeper/secret", &mut known));
        // The outermost link is the one named, whatever lies beyond it.
        fs::create_dir_all(scratch.0.join("elsewhere/inner-target")).unwrap();
        std::os::unix::fs::symlink(
            scratch.0.join("elsewhere/inner-target"),
            scratch.0.join("elsewhere/inner"),
        )
        .unwrap();
        let mut known = HashMap::new();
        assert_eq!(
            link_above(&root, b"a/link/inner/secret", &mut known),
            Some(&b"a/link"[..])
        );
        assert_eq!(link_above(&root, b"a/other/file", &mut known), None);
        assert_eq!(link_above(&root, b"a/link", &mut known), None);
    }
}
