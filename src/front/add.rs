//! `add`: its table, the forms of its operands, its handler, and its lines (G13, G14,
//! G15, `Holds/G13, G14`).
//!
//! What the code below cannot show:
//!
//! - The order is the guard (`Holds/G13, G14`): the hidden paths and one public listing;
//!   the refusal of a publicly tracked file operand; every read-only question, `lstat`
//!   and the private `check-ignore`; then hide-first, the only write of git-dupe's own;
//!   then the runs. A question that fails ends the command with nothing written.
//! - The runs capture no stream: their output, prompts (`-p`, `-e`), and exit status are
//!   Git's. Every pathspec in them is git-dupe's, so they are marked as having their own
//!   paths and no pathspec setting before `dupe` reaches them (G10, G19). Their command
//!   word is the user's, so they carry `-c help.autocorrect=0` (`runner`).
//! - A directory to hide is hidden by `hide::hide_paths`, the one operation that hides a
//!   path, which decides its refusals and prints its `hint:`.

use std::collections::{BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::hide::{self, Hiding};
use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::table::{Entry, Fault, Read, Value};
pub use crate::guards::add::Operand;
use crate::guards::add::{self as decision, Found, Pathspecs, Scoped};
use crate::guards::operand::{self, Cleaned};
use crate::guards::pathspec;
use crate::keeper::{self, Failed, GITDUPE};
use crate::runner::locate::Workspace;
use crate::runner::{End, Failure, Run, records};

const DRY_RUN: Entry = Entry::both(b'n', "dry-run", Value::None);
const PATCH: Entry = Entry::both(b'p', "patch", Value::None);
const EDIT: Entry = Entry::both(b'e', "edit", Value::None);
const FORCE: Entry = Entry::both(b'f', "force", Value::None);
const UPDATE: Entry = Entry::both(b'u', "update", Value::None);
const RENORMALIZE: Entry = Entry::long("renormalize", Value::None);
const ALL: Entry = Entry::both(b'A', "all", Value::None);
const NO_ALL: Entry = Entry::long("no-all", Value::None);
const IGNORE_REMOVAL: Entry = Entry::long("ignore-removal", Value::None);
const NO_IGNORE_REMOVAL: Entry = Entry::long("no-ignore-removal", Value::None);
const REFRESH: Entry = Entry::long("refresh", Value::None);

/// P1's table for `add`.
pub const TABLE: [Entry; 17] = [
    DRY_RUN,
    Entry::both(b'v', "verbose", Value::None),
    PATCH,
    EDIT,
    FORCE,
    UPDATE,
    RENORMALIZE,
    Entry::both(b'N', "intent-to-add", Value::None),
    ALL,
    NO_ALL,
    IGNORE_REMOVAL,
    NO_IGNORE_REMOVAL,
    REFRESH,
    Entry::long("ignore-errors", Value::None),
    Entry::long("ignore-missing", Value::None),
    Entry::long("sparse", Value::None),
    Entry::long("chmod", Value::Next),
];

/// The words of the all-flag, last-wins: the first two set it, the others clear it.
const ALL_FLAG: [&Entry; 4] = [&ALL, &NO_IGNORE_REMOVAL, &NO_ALL, &IGNORE_REMOVAL];

/// Any of these, in either spelling, and the ignore question is not asked.
const NOT_ASKED: [&Entry; 8] = [
    &FORCE,
    &UPDATE,
    &PATCH,
    &EDIT,
    &REFRESH,
    &RENORMALIZE,
    &NO_ALL,
    &IGNORE_REMOVAL,
];

/// Whether the all-flag is set: the last of its words sets it.
fn all_flag(read: &Read) -> bool {
    read.last_of(&ALL_FLAG)
        .is_some_and(|last| *last == ALL || *last == NO_IGNORE_REMOVAL)
}

/// The operands, each a form or a literal path: `.` and `./` the directory form at the
/// user's directory, one that resolves to the root the root form, any other the path it
/// resolves to. An operand that resolves outside the working tree is the fault.
pub fn resolve<'w>(
    workspace: &Workspace,
    operands: &[&'w OsStr],
) -> Result<Vec<Operand>, Fault<'w>> {
    operands
        .iter()
        .map(|&word| {
            let directory = matches!(word.as_bytes(), b"." | b"./");
            match operand::resolve_rooted(word.as_bytes(), workspace.prefix(), workspace.root()) {
                Cleaned::Inside(path) if directory => Ok(Operand::Form(path)),
                Cleaned::Inside(path) => Ok(Operand::Literal(path)),
                Cleaned::Root => Ok(Operand::Form(Vec::new())),
                Cleaned::Outside(_) => Err(Fault::Outside(word)),
            }
        })
        .collect()
}

/// The `add` handler. `words` are the words after `add` as typed, run unchanged when
/// there is no operand and the all-flag is clear.
pub fn add(
    workspace: &Workspace,
    read: &Read,
    words: &[OsString],
    operands: &[Operand],
) -> Outcome {
    let hidden = match keeper::hidden(workspace) {
        Ok(hidden) => hidden,
        Err(failed) => return outcome::failed(failed),
    };
    let scope = match decision::scope(
        &hidden.paths.region,
        &hidden.paths.hidden,
        GITDUPE,
        operands,
        all_flag(read),
    ) {
        Scoped::Unchanged => return unchanged(workspace, words),
        Scoped::NothingHidden => {
            lines::write(
                Level::Hint,
                b"no hidden path but .gitdupe lies there; 'git dupe add <path>' makes a path \
                  private",
            );
            return Outcome::Answered;
        }
        Scoped::Scope(scope) => scope,
    };
    // `.gitdupe` keeps the query from being empty, which would list the whole index.
    let mut query: Vec<Vec<u8>> = scope.paths().map(<[u8]>::to_vec).collect();
    if !query.iter().any(|path| path == GITDUPE) {
        query.push(GITDUPE.to_vec());
    }
    let publicly_tracked = match keeper::publicly_tracked(workspace, &query) {
        Ok(tracked) => tracked,
        Err(failed) => return outcome::failed(failed),
    };
    let privately_tracked = &hidden.paths.privately_tracked;
    let skipped = decision::skipped(&scope, &publicly_tracked, privately_tracked);
    if let Some(path) = decision::refused(&scope, &skipped) {
        return outcome::refuse(
            &[
                path,
                &b" is tracked by the project's Git, and git dupe add never stages such a \
                   path; "[..],
                &hide::route_to_private(Some(path)),
            ]
            .concat(),
        );
    }

    let root = workspace.root();
    let to_hide: Vec<Vec<u8>> = scope
        .literals
        .iter()
        .filter(|path| !hidden.paths.hides(path) && directory(root, path))
        .cloned()
        .collect();
    let found = found(root, &scope.forms, &skipped);
    let updating = read.spells(&UPDATE) || read.spells(&REFRESH);
    let mut pathspecs = Pathspecs::kept(&scope, &skipped, privately_tracked, &found, updating);
    if !scope.forms.is_empty() && !NOT_ASKED.iter().any(|entry| read.spells(entry)) {
        let ignored = match ignored(workspace, &pathspecs.asked(&scope)) {
            Ok(ignored) => ignored,
            Err(ended) => return ended,
        };
        pathspecs = pathspecs.answered(&scope, &ignored, privately_tracked);
    }

    if !to_hide.is_empty() {
        let hiding = if read.spells(&DRY_RUN) {
            Hiding::DryRun
        } else {
            Hiding::Add
        };
        if let Err(ended) = hide::hide_paths(workspace, &to_hide, hidden, &publicly_tracked, hiding)
        {
            return ended;
        }
    }

    let first = read.options().map(OsStr::to_owned).collect();
    let second = read
        .options_without(&ALL_FLAG)
        .into_iter()
        .chain([OsString::from("-u")])
        .collect();
    let mut status = End::Code(0);
    for (options, paths) in [(first, &pathspecs.first), (second, &pathspecs.second)] {
        if paths.is_empty() {
            continue;
        }
        match staging(workspace, options, paths, &pathspecs.exclusions) {
            Ok(End::Code(0)) => {}
            Ok(End::Code(code)) => {
                if status == End::Code(0) {
                    status = End::Code(code);
                }
            }
            Ok(signal) => return Outcome::Git(signal),
            Err(ended) => return ended,
        }
    }
    if !skipped.is_empty() {
        lines::write(Level::Warning, &skipped_line(skipped.len()));
    }
    Outcome::Git(status)
}

/// The run of the user's words unchanged.
fn unchanged(workspace: &Workspace, words: &[OsString]) -> Outcome {
    let private = workspace.private_directory();
    let words = [OsStr::new("add")]
        .into_iter()
        .chain(words.iter().map(OsString::as_os_str));
    match Run::private(&private, workspace.root(), words)
        .own_paths()
        .users_command()
        .start()
    {
        Ok(finished) => Outcome::Git(finished.end),
        Err(failure) => outcome::not_started(failure),
    }
}

/// One `git add` with `options`, `--`, a pathspec per path, and an exclusion per
/// excluded path, from the user's directory with every stream inherited.
fn staging(
    workspace: &Workspace,
    options: Vec<OsString>,
    paths: &[Vec<u8>],
    excluded: &[Vec<u8>],
) -> Result<End, Outcome> {
    let mut words: Vec<OsString> = vec!["add".into()];
    words.extend(options);
    words.push("--".into());
    words.extend(paths.iter().map(|path| pathspec::top_literal(path)));
    words.extend(
        excluded
            .iter()
            .map(|path| pathspec::top_literal_exclude(path)),
    );
    let private = workspace.private_directory();
    match Run::private(&private, workspace.root(), words)
        .own_paths()
        .users_command()
        .start()
    {
        Ok(finished) => Ok(finished.end),
        Err(Failure::TooLong) => Err(outcome::list_too_long(paths.len() + excluded.len())),
        Err(failure) => Err(outcome::not_started(failure)),
    }
}

/// Whether `lstat` finds a directory at `path`, a symbolic link to one being no directory.
fn directory(root: &Path, path: &[u8]) -> bool {
    fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_ok_and(|found| found.is_dir())
}

/// What `lstat` finds for the forms' paths and the skipped paths.
fn found(root: &Path, forms: &[Vec<u8>], skipped: &[Vec<u8>]) -> Found {
    let mut known = HashMap::new();
    let mut found = Found::default();
    for path in forms.iter().chain(skipped) {
        if keeper::beyond_a_link(root, path, &mut known) {
            found.beyond_a_link.insert(path.clone());
        }
    }
    for path in forms {
        if fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_err() {
            found.absent.insert(path.clone());
        }
    }
    found
}

/// The paths among `paths` that the private repository's ignore rules ignore, from one
/// private `git check-ignore --no-index -v -n -z --stdin` from the root (S5). Exit 1
/// means none is; any other failure ends the command.
fn ignored(workspace: &Workspace, paths: &[&[u8]]) -> Result<BTreeSet<Vec<u8>>, Outcome> {
    if paths.is_empty() {
        return Ok(BTreeSet::new());
    }
    let private = workspace.private_directory();
    let words = ["check-ignore", "--no-index", "-v", "-n", "-z", "--stdin"];
    let run = Run::private(&private, workspace.root(), words)
        .from(workspace.root())
        .own_paths()
        .feed(records::ignore_question(paths))
        .capture_output()
        .start()
        .map_err(|failure| {
            outcome::failed(match failure {
                Failure::TooLong => Failed::TooLong(paths.len()),
                failure => Failed::NotStarted(failure),
            })
        })?;
    if run.end != End::Code(0) && run.end != End::Code(1) {
        return Err(Outcome::Git(run.end));
    }
    let Some(records) = records::ignore_records(&run.stdout, paths) else {
        return Err(outcome::refuse(
            b"cannot read Git's answer about which paths the private repository ignores",
        ));
    };
    Ok(paths
        .iter()
        .zip(records)
        .filter(|(_, record)| record.ignored())
        .map(|(path, _)| path.to_vec())
        .collect())
}

/// The `warning:` giving the count of the skipped paths (G14).
fn skipped_line(count: usize) -> Vec<u8> {
    let paths = match count {
        1 => b"1 path the project's Git tracks was".to_vec(),
        _ => format!("{count} paths the project's Git tracks were").into_bytes(),
    };
    [
        &paths[..],
        b" left unstaged; ",
        &hide::route_to_private(None),
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::table::{self, Reading};

    fn read_add(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    fn all(words: &[&str]) -> bool {
        let words = read_add(words);
        match table::read(&TABLE, &words) {
            Reading::Read(read) => all_flag(&read),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_all_flag_is_its_last_word_alone_or_bundled() {
        assert!(all(&["-A"]));
        assert!(all(&["--all"]));
        assert!(all(&["--no-ignore-removal"]));
        assert!(all(&["-nA"]));
        assert!(all(&["--no-all", "-A"]));
        assert!(all(&["--ignore-removal", "-A"]));
        assert!(all(&["-A", "--no-all", "-vA"]));
        assert!(!all(&["-A", "--no-all"]));
        assert!(!all(&["-A", "--ignore-removal"]));
        assert!(!all(&["-nv"]));
        // A value taken as the next word spells nothing.
        assert!(!all(&["--chmod", "-A"]));
        assert!(all(&["--chmod", "+x", "-A"]));
    }

    #[test]
    fn the_second_run_has_the_words_without_the_all_flag() {
        let words = read_add(&["-Av", "--chmod", "+x", "--no-ignore-removal", "-n", "--all"]);
        let Reading::Read(read) = table::read(&TABLE, &words) else {
            panic!()
        };
        assert_eq!(
            read.options_without(&ALL_FLAG),
            read_add(&["-v", "--chmod", "+x", "-n"])
        );
    }

    #[test]
    fn a_count_names_one_path_or_several() {
        assert!(skipped_line(1).starts_with(b"1 path the project's Git tracks was left"));
        assert!(skipped_line(3).starts_with(b"3 paths the project's Git tracks were left"));
    }
}
