//! `clean`: its table, its handler, and its line (G16, `Holds/G16`).
//!
//! What the code below cannot show:
//!
//! - The run is Git's own `clean` in the public repository, from the user's directory,
//!   every stream inherited, with the user's words as typed and in order: in the private
//!   repository every file of the project is untracked, and `clean` there deletes the
//!   project (R1, G10). It is the one public run that writes (R2). Its command word is
//!   the user's, so it carries `-c help.autocorrect=0` (`runner`).
//! - The user's operands are Git's pathspecs, read by Git, and by git-dupe only by their
//!   bytes, for the refusal of a non-literal one (`guards::clean::non_literal`): the run
//!   is not marked as having git-dupe's own paths, so a pathspec setting before `dupe`
//!   governs them (G19). git-dupe adds no pathspec of its own; it adds `-e` patterns,
//!   before the user's `--` when there is one and after every other word otherwise, so
//!   that they are the later and deciding patterns (`guards::clean`).
//! - The order is the guard: the hidden paths, and the refusal while `.gitdupe` is
//!   unreadable, in every form, because the paths to spare cannot be read (F5, G16), so
//!   that the one line a developer sees names what they can repair; the refusal while a
//!   hidden path has a symbolic link among its ancestors, in every form, `-X` beside `-e`
//!   included, because Git never looks beyond the link, and can delete the link itself as
//!   one entry, the hidden path's only way to what stands at it, without following it
//!   (G16, S5); the refusal from the words alone; then `lstat` of the region paths'
//!   ancestors and, under `-X` without `-d` or a pathspec, of the region paths, and the
//!   one public listing under the ancestors holding a `.git` entry, those region paths at
//!   which a directory or a symbolic link stands, and, given a non-literal pathspec, the
//!   outermost ancestor of each region path, when there is one of any; the refusal of a non-literal pathspec while a
//!   region path lies below a directory under which that listing names nothing, decided
//!   before the run because Git's `clean` can delete such a directory whole, the hidden
//!   path with it (G16, S4); the words alone when the last one is an `-e` lacking its
//!   value, which Git refuses before deleting anything; under `-X` the one ignore
//!   question, any failure of which ends the command before `clean` runs; then the run.
//!   No Git run is made per path, and the only per-path work here is `lstat` (R4, G22).
//! - The listing reads the index the `clean` run reads, from the same directory: a hook's
//!   temporary index when `GIT_INDEX_FILE` names one (S9), else the public index. Which
//!   directory holding a `.git` entry Git takes whole, which directory at a hidden path
//!   it enters, and which directory it reads whole for a pathspec depend on that index
//!   (S4), so the two must agree on it (`Holds/G16`). It is asked in every spelling of
//!   its query's ASCII case, because Git's `clean` enters a directory under any spelling
//!   where it reads ignore rules under `core.ignoreCase` (S18), and on Windows it enters
//!   a junction (S24); every comparison of what it names stays one of bytes.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::Path;

use super::lines;
use super::outcome::{self, Outcome};
use super::table::{Entry, Read, Value};
use crate::bytes;
use crate::guards::clean::{self as patterns, Arrangement};
use crate::guards::operand;
use crate::keeper;
use crate::runner::locate::{Workspace, directory_at, nested_repository_at};
use crate::runner::{Failure, Run, Word};

const DIRECTORIES: Entry = Entry::short(b'd', Value::None);

const ONLY_IGNORED: Entry = Entry::short(b'X', Value::None);

const EXCLUDE: Entry = Entry::both(b'e', "exclude", Value::Next);

/// P1's table for `clean`.
pub const TABLE: [Entry; 8] = [
    Entry::both(b'q', "quiet", Value::None),
    Entry::both(b'n', "dry-run", Value::None),
    Entry::both(b'f', "force", Value::None),
    Entry::both(b'i', "interactive", Value::None),
    DIRECTORIES,
    Entry::short(b'x', Value::None),
    ONLY_IGNORED,
    EXCLUDE,
];

/// The refusal of `-X` beside `-e` (G16).
const REFUSED: &[u8] = b"git dupe clean takes -X or -e, not both: under -X it spares the \
    hidden paths through exclude patterns of its own";

/// The `clean` handler. `words` are the words after `clean` as typed.
pub fn clean(workspace: &Workspace, read: &Read, words: &[Vec<u8>]) -> Outcome {
    let hidden = match keeper::hidden(workspace) {
        Ok(hidden) => hidden,
        Err(failed) => return outcome::failed(failed),
    };
    if let Some(cause) = hidden.gitdupe.unreadable() {
        return outcome::refuse(&unreadable(cause));
    }
    let root = workspace.root();
    let mut known = HashMap::new();
    let link_above = |path| keeper::link_above(root, path, &mut known);
    let hidden_paths = hidden.paths.hidden.iter().map(Vec::as_slice);
    if let Some((path, link)) = patterns::refused_beyond_a_link(hidden_paths, link_above) {
        return outcome::refuse(&beyond_a_link(path, link));
    }
    let only_ignored = read.spells(&ONLY_IGNORED);
    if patterns::refused(only_ignored, read.spells(&EXCLUDE)) {
        return outcome::refuse(REFUSED);
    }
    let region = &hidden.paths.region;
    let non_literal = read
        .operands
        .iter()
        .find(|word| patterns::non_literal(word));
    let keep_directories_whole =
        only_ignored && !read.spells(&DIRECTORIES) && read.operands.is_empty();
    let holding_git = holding_a_git_entry(root, region);
    let entered = if keep_directories_whole {
        entered_at(root, region)
    } else {
        BTreeSet::new()
    };
    let mut query = holding_git.clone();
    query.extend(entered.iter().cloned());
    if non_literal.is_some() {
        query.extend(patterns::outermost_ancestors(region));
    }
    let tracked = if query.is_empty() {
        BTreeSet::new()
    } else {
        let query: Vec<Vec<u8>> = query.iter().cloned().collect();
        match keeper::tracked_in_the_users_index(&query) {
            Ok(tracked) => tracked,
            Err(failed) => return outcome::failed(failed),
        }
    };
    if let Some(word) = non_literal
        && let Some((path, directory)) = patterns::loose(region, &tracked)
    {
        return outcome::refuse(&over_a_loose_directory(word, directory, path, only_ignored));
    }
    // After the refusals, which hold in every form: Git refuses these words before it
    // deletes anything, and a pattern of git-dupe's after them would be their value.
    if read.lacking_a_value() == Some(&EXCLUDE) {
        return run(words.iter().map(Word::from).collect(), 0);
    }
    let nested = patterns::nested(&holding_git, &tracked);
    let spared = patterns::spared(region, &nested);
    let ignored_ancestors = if only_ignored {
        let asked = askable(root, &patterns::ancestors(&spared));
        let asked: Vec<&[u8]> = asked.iter().map(Vec::as_slice).collect();
        match keeper::publicly_ignored(workspace, &asked) {
            Ok(ignored) => ignored,
            Err(failed) => return outcome::failed(failed),
        }
    } else {
        BTreeSet::new()
    };
    let arrangement = if only_ignored {
        Arrangement::Unignored {
            ignored_ancestors: &ignored_ancestors,
            keep_directories_whole: keep_directories_whole
                && !patterns::case_reached(&entered, &tracked),
            publicly_tracked: &tracked,
        }
    } else {
        Arrangement::Excluded
    };
    let added = patterns::patterns(&spared, &arrangement, keeper::rule);

    // Before the `--` that ended the reading, else after every word.
    let at = read.end_of_options().unwrap_or(words.len());
    run(
        with_patterns(words, at, added),
        spared.len() + ignored_ancestors.len(),
    )
}

/// `words` with an `-e` and a pattern, each naming the path its rule is written from, for
/// each of `added`, at `at`.
fn with_patterns(words: &[Vec<u8>], at: usize, added: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<Word> {
    let options = added
        .into_iter()
        .flat_map(|(pattern, path)| [Word::from("-e"), Word::naming(pattern, &path)]);
    words[..at]
        .iter()
        .map(Word::from)
        .chain(options)
        .chain(words[at..].iter().map(Word::from))
        .collect()
}

/// The refusal while `.gitdupe` cannot be read, naming it and `cause` (F5, G16).
fn unreadable(cause: &io::Error) -> Vec<u8> {
    [
        b".gitdupe cannot be read as a file (",
        cause.to_string().as_bytes(),
        b"); git dupe clean cannot tell what to spare and deleted nothing",
    ]
    .concat()
}

/// The refusal while the hidden path `path` has the symbolic link `link`, the outermost,
/// among its ancestors (G16). It says nothing of what Git does beyond the link, which on
/// Windows may be a junction Git walks (S24).
fn beyond_a_link(path: &[u8], link: &[u8]) -> Vec<u8> {
    [
        b"git dupe clean cannot spare the hidden path ",
        path,
        b", which lies beyond the symbolic link ",
        link,
        b", and deleted nothing",
    ]
    .concat()
}

/// The refusal of the non-literal pathspec `word`, as typed, while the hidden path `path`
/// lies below the loose directory `directory`, its outermost (G16). Under `-X`, which
/// refuses `-e`, the way out is literal pathspecs alone.
fn over_a_loose_directory(
    word: &[u8],
    directory: &[u8],
    path: &[u8],
    only_ignored: bool,
) -> Vec<u8> {
    let way_out: &[u8] = if only_ignored {
        b"; give literal pathspecs instead"
    } else {
        b"; give literal pathspecs instead, and -e <pattern> for what to keep"
    };
    [
        b"git dupe clean cannot spare the hidden path ",
        path,
        b" given the pathspec ",
        &lines::typed(word),
        b", which is not a literal path: Git's clean can delete ",
        directory,
        b", which is not hidden and holds nothing the project tracks, whole with it",
        way_out,
    ]
    .concat()
}

/// The one public `git clean` with `words`, whose `-e` patterns were written for
/// `carried` paths: the count a refusal names when the words do not fit (G23).
fn run(words: Vec<Word>, carried: usize) -> Outcome {
    let words = [Word::from("clean")].into_iter().chain(words);
    match Run::public(words).users_command().start() {
        Ok(finished) => Outcome::Git(finished.end),
        Err(Failure::TooLong) => outcome::list_too_long(carried),
        Err(failure) => outcome::not_started(failure),
    }
}

/// The ancestor directories of the region paths that hold a `.git` entry, by `lstat`,
/// whether or not Git would open it: each a nested repository as G16 counts it unless
/// the project tracks a path in it (`guards::clean::nested`). Each region path's
/// ancestors are looked at outermost first, and the look stops at one that `lstat` does
/// not find to be a directory: a symbolic link is one entry to Git, which never looks
/// beyond it, and `<link>/.git` would be looked up through the link.
fn holding_a_git_entry(root: &Path, region: &[Vec<u8>]) -> BTreeSet<Vec<u8>> {
    let mut looked: HashMap<&[u8], Found> = HashMap::new();
    let mut nested = BTreeSet::new();
    for path in region {
        for ancestor in operand::ancestors(path) {
            let found = *looked
                .entry(ancestor)
                .or_insert_with(|| found_at(root, ancestor));
            match found {
                Found::NotADirectory => break,
                Found::Repository => {
                    nested.insert(ancestor.to_vec());
                }
                Found::Directory => {}
            }
        }
    }
    nested
}

#[derive(Clone, Copy)]
enum Found {
    NotADirectory,
    Directory,
    /// A directory holding a `.git` entry of any kind, a dangling link included.
    Repository,
}

fn found_at(root: &Path, ancestor: &[u8]) -> Found {
    if !directory_at(root, ancestor) {
        Found::NotADirectory
    } else if nested_repository_at(root, ancestor) {
        Found::Repository
    } else {
        Found::Directory
    }
}

/// The region paths at which `lstat` finds a directory or a symbolic link, a junction
/// among them on Windows, which `lstat` calls a link and Git walks as a directory (S24):
/// under `-X` without `-d` or a pathspec, the listing says under which of them the index
/// the `clean` run reads holds a path, so that Git, which enters such a directory whatever
/// the patterns say (S4), finds nothing below it ignored (`guards::clean::patterns`). A
/// link it does not enter is kept by its own rule, whatever the listing says.
fn entered_at(root: &Path, region: &[Vec<u8>]) -> BTreeSet<Vec<u8>> {
    region
        .iter()
        .filter(|path| {
            bytes::under(root, path)
                .and_then(|path| fs::symlink_metadata(path).ok())
                .is_some_and(|found| found.is_dir() || found.is_symlink())
        })
        .cloned()
        .collect()
}

/// The ancestors public Git can be asked about, and so lifted: each at which `lstat` finds
/// a directory or nothing, and that lies beyond no symbolic link, in byte order. A link
/// among a path's ancestors makes the whole question fail (S5); a link, a file, or
/// anything else that is not a directory, standing at an ancestor's path, is one entry
/// to Git, holds nothing to open, and lifting it would un-ignore that entry itself.
fn askable(root: &Path, ancestors: &BTreeSet<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut known = HashMap::new();
    ancestors
        .iter()
        .filter(|ancestor| {
            let other_than_a_directory = bytes::under(root, ancestor)
                .and_then(|ancestor| fs::symlink_metadata(ancestor).ok())
                .is_some_and(|found| !found.is_dir());
            !other_than_a_directory && !keeper::beyond_a_link(root, ancestor, &mut known)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::table::{self, Reading};

    fn read(words: &[&str]) -> Read<'static> {
        let words: &'static [Vec<u8>] = Box::leak(
            words
                .iter()
                .map(|word| word.as_bytes().to_vec())
                .collect::<Box<[_]>>(),
        );
        match table::read(&TABLE, words) {
            Reading::Read(read) => read,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn x_beside_e_is_found_in_every_spelling_and_e_x_is_no_x() {
        for words in [
            &["-fX", "-e", "x"][..],
            &["-fX", "-ex"],
            &["-fX", "--exclude=x"],
            &["-fX", "--exclude", "x"],
            &["-fXe", "x"],
            &["-e", "x", "-fX"],
            &["-nX", "-e", "x"],
            &["-fX", "-e"],
        ] {
            let read = read(words);
            assert!(
                patterns::refused(read.spells(&ONLY_IGNORED), read.spells(&EXCLUDE)),
                "{words:?}"
            );
        }
        let read = read(&["-n", "-eX"]);
        assert!(!read.spells(&ONLY_IGNORED));
        assert!(read.spells(&EXCLUDE));
    }

    #[test]
    fn an_exclude_typed_last_lacks_its_value_in_every_spelling() {
        for words in [
            &["-f", "-x", "-e"][..],
            &["-fxe"],
            &["-f", "-x", "--exclude"],
        ] {
            assert_eq!(read(words).lacking_a_value(), Some(&EXCLUDE), "{words:?}");
        }
        for words in [
            &["-f", "-e", "x"][..],
            &["-f", "-e", "--"],
            &["-fex"],
            &["--exclude="],
            &["-f", "--", "-e"],
        ] {
            assert_eq!(read(words).lacking_a_value(), None, "{words:?}");
        }
    }

    #[test]
    fn the_users_end_of_options_is_never_the_value_of_e() {
        let given = read(&["-f", "-e", "--", "--", "x"]);
        assert_eq!(given.end_of_options(), Some(3));
        assert_eq!(given.operands, ["x".as_bytes()]);
        assert_eq!(read(&["-f", "x"]).end_of_options(), None);
    }

    #[test]
    fn a_spared_path_that_is_no_name_refuses_the_run_naming_the_path_and_not_its_pattern() {
        let rule = keeper::rule(b"notes/\xff");
        let added = vec![
            ([b"!", &rule[..]].concat(), b"notes/\xff".to_vec()),
            ([&rule[..], b"/**"].concat(), b"notes/\xff".to_vec()),
        ];
        let words = with_patterns(&[b"-n".to_vec(), b"-X".to_vec()], 2, added);
        let run = Run::public(words).capture_output().capture_errors().start();
        match run {
            Err(Failure::NotAName(path)) => assert_eq!(path, b"notes/\xff"),
            _ => assert!(crate::links_and_modes::names_every_byte_string()),
        }
    }

    #[test]
    fn no_long_word_spells_a_letter_without_a_long_name() {
        let words: Vec<Vec<u8>> = ["--d", "--X", "--x", "--=x", "--"].map(Vec::from).to_vec();
        for word in &words[..4] {
            assert_eq!(
                table::read(&TABLE, std::slice::from_ref(word)),
                Reading::NotHeld(word),
                "{word:?}"
            );
        }
        assert!(matches!(table::read(&TABLE, &words[4..]), Reading::Read(_)));
    }

    #[test]
    fn the_words_the_table_does_not_hold_are_named() {
        for word in [
            "--dry",
            "--end-of-options",
            "--no-quiet",
            "--interactive=1",
            "-fZ",
            "--exclude-standard",
            "--no-exclude",
            "--force=1",
        ] {
            let words = [b"-f".to_vec(), word.as_bytes().to_vec()];
            assert_eq!(
                table::read(&TABLE, &words),
                Reading::NotHeld(word.as_bytes()),
                "{word}"
            );
        }
    }

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_git_entry_counts_whatever_it_is_and_a_link_is_never_looked_through() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-clean-{}", std::process::id())));
        let _ = fs::remove_dir_all(&scratch.0);
        let root = scratch.0.join("root");
        for directory in [
            "opened/.git",
            "file/in",
            "dangling",
            "outer/inner/.git",
            "plain/a",
        ] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(root.join("file/.git"), b"").unwrap();
        crate::links_and_modes::symlink(scratch.0.join("nowhere"), root.join("dangling/.git"))
            .unwrap();
        fs::create_dir_all(root.join("outer/.git")).unwrap();
        // A link to a directory that holds a `.git` entry is no nested repository.
        fs::create_dir_all(scratch.0.join("target/.git")).unwrap();
        crate::links_and_modes::symlink(scratch.0.join("target"), root.join("link")).unwrap();
        // A file standing at an ancestor's path ends the look.
        fs::write(root.join("blocked"), b"").unwrap();

        let region: Vec<Vec<u8>> = [
            "opened/secret",
            "file/in/secret",
            "dangling/secret",
            "outer/inner/secret",
            "plain/a/secret",
            "link/secret",
            "blocked/secret",
            "top",
        ]
        .iter()
        .map(|path| path.as_bytes().to_vec())
        .collect();
        let holding: Vec<Vec<u8>> = holding_a_git_entry(&root, &region).into_iter().collect();
        let expected = ["dangling", "file", "opened", "outer", "outer/inner"];
        assert_eq!(holding, expected.map(|path| path.as_bytes().to_vec()));

        let ancestors: BTreeSet<Vec<u8>> = ["link", "link/sub", "plain", "plain/a", "absent"]
            .iter()
            .map(|path| path.as_bytes().to_vec())
            .collect();
        assert_eq!(
            askable(&root, &ancestors),
            [b"absent".to_vec(), b"plain".to_vec(), b"plain/a".to_vec()]
        );
    }
}
