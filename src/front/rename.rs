//! The hint after a `git dupe mv` that moved a directory `.gitdupe` lists (`Holds/G19`,
//! G25): Git moves the directory with every file in it, and `.gitdupe` goes on listing the
//! old name, so that what the private repository does not track under the new name is
//! visible to public Git and missing from `git dupe status`. The hint names the two
//! commands that move the listing with it, `git dupe hide` of the new name and then
//! `git dupe unhide` of the old one; the manual page gives the order that exposes nothing.
//!
//! What the code below cannot show:
//!
//! - It answers no advice of Git's, so it is decided from the words, the run's end, the
//!   list, the public index, and `lstat`, never from Git's text (R5): after a run that
//!   exited 0, for each source operand that `.gitdupe` lists as it is, that stood before
//!   the run and stands no more, and whose new place, where nothing stood before the run,
//!   is a directory, at or below no listed path. A dry run, a help request before `--`,
//!   and a run that did not exit 0 get none, and `git dupe git mv` never comes here (G20).
//!   A source below a listed directory stays hidden by it, and a new place at or below a
//!   listed path is hidden already: neither is named. A source a `mv -k` skipped still
//!   stands, or never stood, and is not named either.
//! - Git's reading of the words is followed only as far as the hint needs: no option of
//!   `mv` takes a value, so a dashed word before `--` other than `-` is an option, and every
//!   other word an operand, the last one the destination. Where the destination stood as
//!   a directory before the run, or several sources were given, Git moved each source into
//!   it under its last component. A dry run is `--dry-run` or a prefix of it Git reads as
//!   one, or a word of short options holding `n`, unless a later `--no-dry-run`, or a
//!   prefix of it Git reads as one, undoes it.
//! - The list is the keeper's, read once after the run, which runs Git only where nothing
//!   stands at `.gitdupe` and its staged version is read; where a listed directory moved,
//!   one public listing under the new places follows (G22). Every other question is an
//!   `lstat` of a path the words name.
//! - Where the project's Git tracks a path at or below a new place, `hide` refuses it
//!   (G11): the hint names instead the `git dupe mv` of each new place back to its old
//!   name, which the list still hides.
//! - The paths are root-relative and the commands are to be run from the root, as settle's
//!   remedies say of theirs, each path written after `--` as `offered` and `shell_word`
//!   write an operand. A path holding `*`, `?`, or `[`, which `hide` and `unhide` refuse as
//!   a pattern (G11), has no such form: where one is among them, the hint names the edit
//!   of `.gitdupe` instead, and `git dupe add -- .gitdupe` to stage it.

use std::collections::BTreeSet;
use std::path::Path;

use super::lines::{self, Level};
use super::outcome::Outcome;
use crate::guards::operand::{self, Cleaned, NotLiteral};
use crate::guards::quoted::shell_word;
use crate::keeper;
use crate::runner::End;
use crate::runner::locate::{self, Workspace};

/// A source operand and the place Git moves it to, both root-relative.
#[derive(Debug, PartialEq, Eq)]
struct Move {
    from: Vec<u8>,
    to: Vec<u8>,
}

/// What a `mv` may have moved, decided from its words and from what stood at its
/// destination before the run.
pub struct Then(Vec<Move>);

/// What the run of `words`, the command word first, may move in `workspace`: each source
/// that stands, to a new place where nothing stands, before the run.
pub fn before(words: &[Vec<u8>], workspace: &Workspace) -> Then {
    let root = workspace.root();
    let mut found = moves(words, workspace.prefix(), root, |path| {
        locate::directory_at(root, path)
    });
    found.retain(|moved| locate::present(root, &moved.from) && !locate::present(root, &moved.to));
    Then(found)
}

/// Writes the hint after a run that ended as `ended`, where it moved a listed directory to
/// a place nothing hides; a listing that fails gives none.
pub fn after(then: Then, ended: &Outcome, workspace: &Workspace) {
    if then.0.is_empty() || !matches!(ended, Outcome::Git(End::Code(0))) {
        return;
    }
    let listed: BTreeSet<Vec<u8>> = keeper::listed_paths(workspace).into_iter().collect();
    let moved = unlisted_moves(&then.0, &listed, workspace.root());
    if moved.is_empty() {
        return;
    }
    let places: Vec<Vec<u8>> = moved.iter().map(|moved| moved.to.clone()).collect();
    let Ok(tracked) = keeper::publicly_tracked(workspace, &places) else {
        return;
    };
    let refused = moved.iter().any(|moved| {
        tracked
            .iter()
            .any(|path| operand::at_or_below(path, &moved.to))
    });
    lines::write(Level::Hint, &line(&moved, refused));
}

/// The moves of `words` from the user's `prefix`, `directory` saying where a directory
/// stands: none for another command, a help request, or a dry run.
fn moves(
    words: &[Vec<u8>],
    prefix: &[u8],
    root: &Path,
    directory: impl Fn(&[u8]) -> bool,
) -> Vec<Move> {
    let Some((b"mv", rest)) = words
        .split_first()
        .map(|(word, rest)| (word.as_slice(), rest))
    else {
        return Vec::new();
    };
    let mut operands = Vec::new();
    let mut dry = false;
    let mut words = rest.iter().map(Vec::as_slice);
    for word in words.by_ref() {
        match word {
            b"--" => break,
            b"-h" | b"--help" => return Vec::new(),
            _ if dry_run(word) => dry = true,
            _ if word.len() >= 6 && b"--no-dry-run".starts_with(word) => dry = false,
            _ if word.starts_with(b"-") && word != b"-" => {}
            _ => operands.push(word),
        }
    }
    if dry {
        return Vec::new();
    }
    operands.extend(words);
    let Some((destination, sources)) = operands.split_last() else {
        return Vec::new();
    };
    let (destination, into) = match operand::resolve(destination, prefix, root) {
        Cleaned::Inside(path) => {
            let into = sources.len() > 1 || directory(&path);
            (path, into)
        }
        Cleaned::Root => (Vec::new(), true),
        Cleaned::Outside(_) => return Vec::new(),
    };
    sources
        .iter()
        .filter_map(|source| match operand::resolve(source, prefix, root) {
            Cleaned::Inside(from) if into => {
                let name = from.rsplit(|byte| *byte == b'/').next().unwrap_or(&from);
                let to = if destination.is_empty() {
                    name.to_vec()
                } else {
                    [&destination[..], b"/", name].concat()
                };
                Some(Move { from, to })
            }
            Cleaned::Inside(from) => Some(Move {
                from,
                to: destination.clone(),
            }),
            Cleaned::Root | Cleaned::Outside(_) => None,
        })
        .collect()
}

/// Whether `mv` reads `word` as `--dry-run`: the word or a prefix of it at least three
/// bytes long, which no other long option of `mv` begins with, or a word of short options
/// holding `n`.
fn dry_run(word: &[u8]) -> bool {
    match word.strip_prefix(b"--") {
        Some(_) => word.len() >= 3 && b"--dry-run".starts_with(word),
        None => word.starts_with(b"-") && word.contains(&b'n'),
    }
}

/// The moves after the run of a directory `listed` names, which stands no more, to a
/// directory at or below no listed path.
fn unlisted_moves<'m>(moves: &'m [Move], listed: &BTreeSet<Vec<u8>>, root: &Path) -> Vec<&'m Move> {
    moves
        .iter()
        .filter(|moved| {
            listed.contains(&moved.from)
                && !listed
                    .iter()
                    .any(|path| operand::at_or_below(&moved.to, path))
                && !locate::present(root, &moved.from)
                && locate::directory_at(root, &moved.to)
        })
        .collect()
}

/// The hint for `moved`, at least one: where `refused`, the project's Git tracking a path
/// at or below a new place, the move of each back.
fn line(moved: &[&Move], refused: bool) -> Vec<u8> {
    let from: Vec<&[u8]> = moved.iter().map(|moved| moved.from.as_slice()).collect();
    let to: Vec<&[u8]> = moved.iter().map(|moved| moved.to.as_slice()).collect();
    let (it, they_stand) = match moved {
        [_] => (&b"it"[..], &b"it stands"[..]),
        _ => (&b"them"[..], &b"they stand"[..]),
    };
    if refused {
        let backs: Vec<Vec<u8>> = moved
            .iter()
            .map(|moved| {
                [
                    &b"'git dupe mv --"[..],
                    &operands(&[&moved.to, &moved.from]),
                    b"'",
                ]
                .concat()
            })
            .collect();
        let backs: Vec<&[u8]> = backs.iter().map(Vec::as_slice).collect();
        return [
            &listing(&from)[..],
            b" moved to ",
            &listing(&to),
            b", which .gitdupe does not list, so that what the private repository does not \
              track where ",
            they_stand,
            b" now is visible to public Git, and public Git tracks a path there, so that \
              git dupe hide refuses it; run from the root, ",
            &listing(&backs),
            match moved {
                [_] => b" moves it back",
                _ => b" move them back",
            },
        ]
        .concat();
    }
    let mut line = [
        &listing(&from)[..],
        b" moved to ",
        &listing(&to),
        b", which .gitdupe does not list, so that what the private repository does not \
          track where ",
        they_stand,
        b" now is visible to public Git; ",
    ]
    .concat();
    let offerable = from
        .iter()
        .chain(&to)
        .all(|path| operand::literal(&operand::offered(path)) != Err(NotLiteral::Pattern));
    if offerable {
        line.extend_from_slice(b"run from the root, 'git dupe hide --");
        line.extend_from_slice(&operands(&to));
        line.extend_from_slice(b"' and then 'git dupe unhide --");
        line.extend_from_slice(&operands(&from));
        line.extend_from_slice(b"' move the listing with ");
        line.extend_from_slice(it);
    } else {
        line.extend_from_slice(b"edit .gitdupe so that it lists ");
        line.extend_from_slice(&listing(&to));
        line.extend_from_slice(b" in place of ");
        line.extend_from_slice(&listing(&from));
        line.extend_from_slice(b", and run 'git dupe add -- .gitdupe' from the root to stage it");
    }
    line
}

/// `paths` as operands of one command, each after a space.
fn operands(paths: &[&[u8]]) -> Vec<u8> {
    paths
        .iter()
        .flat_map(|path| [&b" "[..], &shell_word(&operand::offered(path))].concat())
        .collect()
}

/// `paths` named in a sentence: joined with commas and a final `and`.
fn listing(paths: &[&[u8]]) -> Vec<u8> {
    match paths {
        [] => Vec::new(),
        [one] => one.to_vec(),
        [first, second] => [first, &b" and "[..], second].concat(),
        [rest @ .., last] => {
            let mut joined: Vec<u8> = rest
                .iter()
                .flat_map(|path| [path, &b", "[..]].concat())
                .collect();
            joined.extend_from_slice(b"and ");
            joined.extend_from_slice(last);
            joined
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> Vec<Vec<u8>> {
        words.iter().map(|word| word.as_bytes().to_vec()).collect()
    }

    fn root() -> &'static Path {
        Path::new("/w")
    }

    /// The moves of `typed` from the root, where `directories` stand.
    fn moved(typed: &[&str], directories: &[&str]) -> Vec<(String, String)> {
        let found = moves(&words(typed), b"", root(), |path| {
            directories
                .iter()
                .any(|directory| directory.as_bytes() == path)
        });
        found
            .into_iter()
            .map(|moved| {
                (
                    String::from_utf8(moved.from).unwrap(),
                    String::from_utf8(moved.to).unwrap(),
                )
            })
            .collect()
    }

    fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(from, to)| (from.to_string(), to.to_string()))
            .collect()
    }

    #[test]
    fn a_source_moves_to_the_destination_or_into_it_where_a_directory_stands() {
        assert_eq!(
            moved(&["mv", "notes", "journal"], &[]),
            pairs(&[("notes", "journal")])
        );
        assert_eq!(
            moved(&["mv", "notes/", "journal"], &["journal"]),
            pairs(&[("notes", "journal/notes")])
        );
        assert_eq!(
            moved(&["mv", "a/notes", "plans", "box"], &[]),
            pairs(&[("a/notes", "box/notes"), ("plans", "box/plans")])
        );
        assert_eq!(
            moved(&["mv", "a/notes", "."], &[]),
            pairs(&[("a/notes", "notes")])
        );
        // Options anywhere before `--`, and everything after it an operand.
        assert_eq!(
            moved(&["mv", "-v", "notes", "-f", "-k", "--", "-x"], &[]),
            pairs(&[("notes", "-x")])
        );
        // From a subdirectory, the paths are the root's.
        let found = moves(
            &words(&["mv", "notes", "../journal"]),
            b"sub/",
            root(),
            |_| false,
        );
        assert_eq!(
            found,
            [Move {
                from: b"sub/notes".to_vec(),
                to: b"journal".to_vec()
            }]
        );
    }

    #[test]
    fn another_command_a_dry_run_a_help_request_or_one_operand_moves_nothing() {
        for typed in [
            &["mv", "-n", "notes", "journal"][..],
            &["mv", "--dry-run", "notes", "journal"],
            &["mv", "--dry", "notes", "journal"],
            &["mv", "--d", "notes", "journal"],
            &["mv", "-vn", "notes", "journal"],
            &["mv", "notes", "journal", "-n"],
            &["mv", "-h"],
            &["mv", "notes", "journal", "--help"],
            &["mv", "notes"],
            &["mv"],
            &["move", "notes", "journal"],
            &["mv", "notes", "../outside"],
            &[],
        ] {
            assert_eq!(moved(typed, &[]), pairs(&[]), "{typed:?}");
        }
        // After `--`, `-n` is a path; a later `--no-dry-run` undoes a dry run, and an earlier
        // one does not.
        assert_eq!(moved(&["mv", "--", "-n", "x"], &[]), pairs(&[("-n", "x")]));
        for typed in [
            &["mv", "-n", "--no-dry-run", "notes", "journal"][..],
            &["mv", "--dry-run", "--no-d", "notes", "journal"],
        ] {
            assert_eq!(
                moved(typed, &[]),
                pairs(&[("notes", "journal")]),
                "{typed:?}"
            );
        }
        assert_eq!(
            moved(&["mv", "--no-dry-run", "-n", "notes", "journal"], &[]),
            pairs(&[])
        );
    }

    #[test]
    fn the_line_names_hide_of_the_new_place_then_unhide_of_the_old() {
        let one = Move {
            from: b"notes".to_vec(),
            to: b"my journal".to_vec(),
        };
        let line = line(&[&one], false);
        let text = String::from_utf8(line).unwrap();
        let hide = text.find("'git dupe hide -- \"my journal\"'").unwrap();
        let unhide = text.find("'git dupe unhide -- notes'").unwrap();
        assert!(hide < unhide, "{text}");
        assert!(text.contains("run from the root"), "{text}");
        assert!(text.ends_with("move the listing with it"), "{text}");
        let two = Move {
            from: b":plans".to_vec(),
            to: b"box/:plans".to_vec(),
        };
        let text = String::from_utf8(super::line(&[&one, &two], false)).unwrap();
        assert!(
            text.contains("'git dupe hide -- \"my journal\" box/:plans'"),
            "{text}"
        );
        assert!(
            text.contains("'git dupe unhide -- notes ./:plans'"),
            "{text}"
        );
        assert!(text.ends_with("move the listing with them"), "{text}");
    }

    #[test]
    fn a_path_hide_would_refuse_as_a_pattern_gets_the_edit_of_gitdupe() {
        let starred = Move {
            from: b"notes".to_vec(),
            to: b"journal[1]".to_vec(),
        };
        let text = String::from_utf8(line(&[&starred], false)).unwrap();
        assert!(!text.contains("git dupe hide"), "{text}");
        assert!(!text.contains("git dupe unhide"), "{text}");
        assert!(
            text.contains("edit .gitdupe so that it lists journal[1] in place of notes"),
            "{text}"
        );
        assert!(text.contains("'git dupe add -- .gitdupe'"), "{text}");
    }

    #[test]
    fn where_hide_would_refuse_the_new_place_the_line_names_each_move_back() {
        let one = Move {
            from: b"notes".to_vec(),
            to: b"journal".to_vec(),
        };
        let text = String::from_utf8(line(&[&one], true)).unwrap();
        assert!(!text.contains("'git dupe hide"), "{text}");
        assert!(text.contains("git dupe hide refuses it"), "{text}");
        assert!(
            text.ends_with("run from the root, 'git dupe mv -- journal notes' moves it back"),
            "{text}"
        );
        let two = Move {
            from: b"a b".to_vec(),
            to: b"box/a b".to_vec(),
        };
        let text = String::from_utf8(line(&[&one, &two], true)).unwrap();
        assert!(
            text.ends_with(
                "'git dupe mv -- journal notes' and 'git dupe mv -- \"box/a b\" \"a b\"' move them back"
            ),
            "{text}"
        );
    }

    #[test]
    fn several_paths_are_listed_with_commas_and_a_final_and() {
        assert_eq!(listing(&[b"a"]), b"a");
        assert_eq!(listing(&[b"a", b"b"]), b"a and b");
        assert_eq!(listing(&[b"a", b"b", b"c"]), b"a, b, and c");
    }
}
