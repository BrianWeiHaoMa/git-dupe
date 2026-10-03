//! `hide`'s and `unhide`'s words, their handlers, and the lines they print.
//!
//! The words are read before anything else (`Composition/Front` step 2): a help request
//! anywhere before `--` is answered without locating. Every other fault is a usage error
//! reported after the locate run and the unattached refusal, and before anything else
//! runs: the faults of the words themselves, found here, then an operand that resolves
//! to the root or outside it, which only the prefix reveals. A line names a path as its
//! cleaned root-relative bytes, never a word as typed.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use crate::guards::hide::{self as decision, Refusal};
use crate::guards::operand::{self, Cleaned, NotLiteral};
use crate::keeper::{self, Edited, GITDUPE, Hidden, NotEdited};
use crate::runner::locate::Workspace;

/// What `hide`'s or `unhide`'s words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked<'w> {
    /// `-h` or `--help` before `--`.
    Help,
    /// The operands, in order, each a literal path as typed.
    Paths(Vec<&'w OsStr>),
    Misused(Fault<'w>),
}

/// Why `hide`'s or `unhide`'s words are a usage error, with the word it is about. A word
/// holding a newline is never named, because no one line can hold it.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault<'w> {
    Option(&'w OsStr),
    NoPath,
    Pattern(&'w OsStr),
    Magic(&'w OsStr),
    Newline,
    Root(&'w OsStr),
    Outside(&'w OsStr),
}

impl Fault<'_> {
    /// The text of the `error:` line: the word as typed, then the fault.
    pub fn text(&self) -> Vec<u8> {
        let (word, fault): (Option<&OsStr>, &[u8]) = match self {
            Fault::Option(word) => (
                Some(word),
                b"is an option, and hide and unhide take none; a path beginning with '-' \
                  goes after '--'",
            ),
            Fault::NoPath => (None, b"hide and unhide need at least one path"),
            Fault::Pattern(word) => (
                Some(word),
                b"holds '*', '?', or '[', and hide and unhide take literal paths",
            ),
            Fault::Magic(word) => (
                Some(word),
                b"begins with ':', and hide and unhide take literal paths",
            ),
            Fault::Newline => (
                None,
                b".gitdupe holds one path per line: a path cannot hold a newline",
            ),
            Fault::Root(word) => (
                Some(word),
                b"names the root of the working tree, which cannot be hidden or unhidden",
            ),
            Fault::Outside(word) => (
                Some(word),
                b"lies outside the working tree, and cannot be hidden or unhidden",
            ),
        };
        match word {
            Some(word) => [b"'", word.as_bytes(), b"' ", fault].concat(),
            None => fault.to_vec(),
        }
    }
}

/// Reads `hide PATH...` or `unhide PATH...`: no option, at least one path, each literal.
/// `--` ends the options, and every word after it is a path.
pub fn read(words: &[OsString]) -> Asked<'_> {
    let end_of_options = words
        .iter()
        .position(|word| word.as_bytes() == b"--")
        .unwrap_or(words.len());
    if words[..end_of_options]
        .iter()
        .any(|word| matches!(word.as_bytes(), b"-h" | b"--help"))
    {
        return Asked::Help;
    }
    let mut paths = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let bytes = word.as_bytes();
        if index == end_of_options {
            continue;
        }
        if bytes.contains(&b'\n') {
            return Asked::Misused(Fault::Newline);
        }
        if index < end_of_options && bytes.starts_with(b"-") {
            return Asked::Misused(Fault::Option(word));
        }
        match operand::literal(bytes) {
            Err(NotLiteral::Pattern) => return Asked::Misused(Fault::Pattern(word)),
            Err(NotLiteral::Magic) => return Asked::Misused(Fault::Magic(word)),
            Ok(()) => paths.push(word.as_os_str()),
        }
    }
    if paths.is_empty() {
        return Asked::Misused(Fault::NoPath);
    }
    Asked::Paths(paths)
}

/// The root-relative paths the operands name, from the user's directory, or the fault of
/// the first that names the root or a path outside it.
pub fn resolve<'w>(workspace: &Workspace, words: &[&'w OsStr]) -> Result<Vec<Vec<u8>>, Fault<'w>> {
    words
        .iter()
        .map(
            |word| match operand::resolve(word.as_bytes(), workspace.prefix(), workspace.root()) {
                Cleaned::Inside(path) => Ok(path),
                Cleaned::Root => Err(Fault::Root(word)),
                Cleaned::Outside(_) => Err(Fault::Outside(word)),
            },
        )
        .collect()
}

/// The `hide` handler: the hidden paths and one public listing under the paths and
/// `.gitdupe`, then the hide.
pub fn hide(workspace: &Workspace, paths: &[Vec<u8>]) -> Outcome {
    let hidden = match keeper::hidden(workspace) {
        Ok(hidden) => hidden,
        Err(failed) => return outcome::failed(failed),
    };
    let query: Vec<Vec<u8>> = paths.iter().cloned().chain([GITDUPE.to_vec()]).collect();
    let publicly_tracked = match keeper::publicly_tracked(workspace, &query) {
        Ok(tracked) => tracked,
        Err(failed) => return outcome::failed(failed),
    };
    match hide_paths(workspace, paths, hidden, &publicly_tracked, Hiding::Hide) {
        Ok(()) => Outcome::Answered,
        Err(ended) => ended,
    }
}

/// The command that hides, which decides what is written and what the lines say.
#[derive(Clone, Copy)]
pub enum Hiding {
    /// `hide`, whose hint also names `git dupe add` as what versions the path.
    Hide,
    /// `add`, which itself versions what it hides: its hint names `git dupe unhide` alone.
    Add,
    /// `add -n`: nothing is written, and each path that would be hidden is named.
    DryRun,
}

/// Hides `paths`: the one operation of every command that hides a path, given the hidden
/// paths and a public listing under `paths` and `.gitdupe` that its caller took. The
/// refusals are decided first, whether or not it writes; then, writing, the keeper adds
/// and stages the lines, and one `hint:` names each newly hidden path; not writing,
/// nothing is written or staged and one `hint:` names each path that would be hidden.
/// `Err` is the outcome the command ends with.
pub fn hide_paths(
    workspace: &Workspace,
    paths: &[Vec<u8>],
    hidden: Hidden,
    publicly_tracked: &BTreeSet<Vec<u8>>,
    hiding: Hiding,
) -> Result<(), Outcome> {
    let privately_tracked = &hidden.paths.privately_tracked;
    if let Some(refusal) = decision::refusal(paths, GITDUPE, privately_tracked, publicly_tracked) {
        return Err(refuse(refusal, publicly_tracked));
    }
    if let Hiding::DryRun = hiding {
        for path in keeper::hiding(&hidden, paths) {
            lines::write(
                Level::Hint,
                &[&path[..], b" would be hidden from the project's Git"].concat(),
            );
        }
        return Ok(());
    }
    // Read before the keeper takes the hidden paths: a path the private repository tracks
    // was hidden before it was listed.
    let tracked: BTreeSet<Vec<u8>> = paths
        .iter()
        .filter(|path| privately_tracked.contains(*path))
        .cloned()
        .collect();
    let root = workspace.root();
    edited(keeper::hide(workspace, hidden, paths), |changed| {
        changed
            .iter()
            .map(|path| newly_hidden(path, tracked.contains(path), hiding, || absent(root, path)))
            .collect()
    })
}

/// The `hint:` naming a newly hidden path and `git dupe unhide`. Under `hide` it also
/// names `git dupe add`, which versions the path, and says when nothing stands there yet,
/// which only `absent` asks; a path the private repository tracks is listed, not newly
/// hidden. Each command names the path, root-relative here, and so is to be run from the
/// root, as `clone`'s lines say of theirs.
fn newly_hidden(path: &[u8], tracked: bool, hiding: Hiding, absent: impl Fn() -> bool) -> Vec<u8> {
    let unhide = [b"'git dupe unhide -- ", path, b"'"].concat();
    if tracked {
        return [
            path,
            b" is now listed in .gitdupe; it was already hidden, because the private \
              repository tracks it, and ",
            &unhide,
            b" run from the root takes it off the list",
        ]
        .concat();
    }
    let add = [b"'git dupe add -- ", path, b"'"].concat();
    let and: Vec<u8> = match hiding {
        Hiding::Hide if absent() => [
            b", though nothing stands there yet; run from the root, ",
            &add[..],
            b" versions it once something does, and ",
            &unhide,
            b" stops hiding it",
        ]
        .concat(),
        Hiding::Hide => [
            b"; run from the root, ",
            &add[..],
            b" versions it, and ",
            &unhide,
            b" stops hiding it",
        ]
        .concat(),
        Hiding::Add | Hiding::DryRun => {
            [b"; run from the root, ", &unhide[..], b" stops hiding it"].concat()
        }
    };
    [path, b" is now hidden from the project's Git", &and].concat()
}

/// Whether nothing stands at `path`, by `lstat`: not found, or a file where a directory
/// above it would be. Any other failure is not taken to mean nothing stands there.
fn absent(root: &Path, path: &[u8]) -> bool {
    fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_err_and(|failed| {
        matches!(
            failed.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        )
    })
}

/// The `unhide` handler: the hidden paths and one public listing for `.gitdupe` alone,
/// then the lines removed, and one `hint:` per distinct path given, in the order first
/// given, whether or not a line named it.
pub fn unhide(workspace: &Workspace, paths: &[Vec<u8>]) -> Outcome {
    let hidden = match keeper::hidden(workspace) {
        Ok(hidden) => hidden,
        Err(failed) => return outcome::failed(failed),
    };
    let publicly_tracked = match keeper::publicly_tracked(workspace, &[GITDUPE.to_vec()]) {
        Ok(tracked) => tracked,
        Err(failed) => return outcome::failed(failed),
    };
    let privately_tracked = &hidden.paths.privately_tracked;
    if let Some(refusal) = decision::gitdupe_refusal(GITDUPE, privately_tracked, &publicly_tracked)
    {
        return refuse(refusal, &publicly_tracked);
    }
    // Read before the keeper takes the hidden paths.
    let mut given: Vec<(&[u8], Stays)> = Vec::new();
    for path in paths {
        if !given.iter().any(|(seen, _)| seen == path) {
            let listed = &hidden.gitdupe.paths;
            given.push((path, stays(listed, privately_tracked, paths, path)));
        }
    }
    let unhidden = edited(keeper::unhide(workspace, hidden, paths), |changed| {
        given
            .iter()
            .map(|(path, stays)| {
                let removed = changed.iter().any(|gone| gone == path);
                unhidden_line(path, removed, stays)
            })
            .collect()
    });
    match unhidden {
        Ok(()) => Outcome::Answered,
        Err(ended) => ended,
    }
}

/// What still hides a path given to `unhide` once the lines naming the given paths are
/// gone, as far as the hidden paths say.
#[derive(Debug, PartialEq, Eq)]
enum Stays {
    /// The private repository tracks the path itself.
    Tracked,
    /// A listed path above it that no given path removes: the outermost.
    Below(Vec<u8>),
    /// The private repository tracks files below it.
    TrackedBelow,
    /// Nothing: whether public Git now sees it is settle's warning to say (G9).
    Nothing,
}

/// What still hides `path`, given the paths `.gitdupe` lists, the privately tracked files,
/// and every path given, whose lines go.
fn stays(
    listed: &[Vec<u8>],
    privately_tracked: &BTreeSet<Vec<u8>>,
    given: &[Vec<u8>],
    path: &[u8],
) -> Stays {
    if privately_tracked.contains(path) {
        return Stays::Tracked;
    }
    let kept_above = operand::ancestors(path).find(|above| {
        listed.iter().any(|line| line == above) && !given.iter().any(|gone| gone == above)
    });
    if let Some(above) = kept_above {
        return Stays::Below(above.to_vec());
    }
    if operand::any_below(privately_tracked, path) {
        return Stays::TrackedBelow;
    }
    Stays::Nothing
}

/// The `hint:` for one path given to `unhide`: whether a line named it, and what keeps it
/// hidden. It never says the path is visible: settle's `warning:` does, where it is.
fn unhidden_line(path: &[u8], removed: bool, stays: &Stays) -> Vec<u8> {
    let head: &[u8] = if removed {
        b" is no longer listed in .gitdupe"
    } else {
        b" is not listed in .gitdupe; there is no line to remove"
    };
    let tail = match stays {
        Stays::Tracked => b"; it stays hidden, because the private repository tracks it".to_vec(),
        Stays::Below(above) => [
            b"; it stays hidden below ",
            &above[..],
            b", which .gitdupe lists",
        ]
        .concat(),
        Stays::TrackedBelow => {
            b"; the files below it that the private repository tracks stay hidden".to_vec()
        }
        Stays::Nothing => Vec::new(),
    };
    [path, head, &tail].concat()
}

/// The lines of an edit: the `hint:` lines `hints` words from the changed paths, a
/// `warning:` when a symbolic link became a regular file, and the end of a staging run
/// that failed, the file written.
fn edited(
    edit: Result<Edited, NotEdited>,
    hints: impl FnOnce(&[Vec<u8>]) -> Vec<Vec<u8>>,
) -> Result<(), Outcome> {
    let edited = match edit {
        Ok(edited) => edited,
        Err(NotEdited::Unreadable(cause)) => {
            return Err(outcome::refuse(
                format!(".gitdupe cannot be read as a file ({cause}); nothing was changed")
                    .as_bytes(),
            ));
        }
        Err(NotEdited::NotWritten(cause)) => {
            return Err(outcome::refuse(
                format!("cannot write .gitdupe ({cause}); nothing was changed").as_bytes(),
            ));
        }
    };
    for hint in hints(&edited.changed) {
        lines::write(Level::Hint, &hint);
    }
    if edited.link_replaced {
        lines::write(
            Level::Warning,
            b".gitdupe was a symbolic link; it is now a regular file holding what its \
              target held and the change, and its target is untouched",
        );
    }
    match edited.not_staged {
        Some(failed) => Err(outcome::failed(failed)),
        None => Ok(()),
    }
}

/// How a path the project's Git tracks becomes private, which `hide`'s refusal and `add`'s
/// refusal and count of skipped paths say (G11, G14): one file at a time, `git rm
/// --cached` first, and then `git dupe add`. The first makes nothing private: it stages
/// the file's deletion from the project, which every other clone receives once it is
/// pushed, and only the second tracks the file privately, because a path is tracked by
/// one repository or the other (N9); a file the project's ignore rules name, which the
/// project's Git can track all the same, then needs `-f` (G15). git-dupe runs neither
/// for the developer (G5). Given a path, root-relative, the commands are written whole,
/// to be run from the root.
pub fn route_to_private(path: Option<&[u8]>) -> Vec<u8> {
    const DELETION: &[u8] = b"a deletion from the project that every other clone \
        receives once it is committed and pushed";
    match path {
        Some(path) => [
            &b"run from the root, 'git rm --cached -- "[..],
            path,
            b"' first, ",
            DELETION,
            b", then 'git dupe add -- ",
            path,
            b"' makes it private, or 'git dupe add -f -- ",
            path,
            b"' where the project ignores it",
        ]
        .concat(),
        None => [
            &b"such a path becomes private one file at a time: 'git rm --cached' of it \
               first, "[..],
            DELETION,
            b", then 'git dupe add' of it, with -f where the project ignores it",
        ]
        .concat(),
    }
}

/// The refusal's `fatal:` line. A path given alone that the project's Git tracks itself
/// is named as the file it is, as `add` names one.
fn refuse(refusal: Refusal, publicly_tracked: &BTreeSet<Vec<u8>>) -> Outcome {
    let line = match refusal {
        Refusal::TracksGitdupe => b"the project's repository tracks .gitdupe and the private \
            repository does not; run 'git rm --cached .gitdupe' first"
            .to_vec(),
        Refusal::PubliclyTracked { count, under } => match &under[..] {
            [file] if count == 1 && publicly_tracked.contains(file) => [
                file,
                &b" is tracked by the project's Git (1 path); "[..],
                &route_to_private(None),
            ]
            .concat(),
            _ => {
                let paths = match count {
                    1 => b"1 path the project's Git tracks lies".to_vec(),
                    _ => format!("{count} paths the project's Git tracks lie").into_bytes(),
                };
                [
                    &paths[..],
                    b" at or below ",
                    &under.join(&b", "[..]),
                    b"; ",
                    &route_to_private(None),
                ]
                .concat()
            }
        },
    };
    outcome::refuse(&line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&[u8]]) -> Vec<OsString> {
        words
            .iter()
            .map(|word| OsStr::from_bytes(word).to_owned())
            .collect()
    }

    fn paths<'w>(words: &[&'w [u8]]) -> Asked<'w> {
        Asked::Paths(words.iter().map(|word| OsStr::from_bytes(word)).collect())
    }

    #[test]
    fn every_word_is_a_path_and_after_the_end_of_options_a_dashed_one_too() {
        for (given, expected) in [
            (&[&b"notes/"[..]][..], &[&b"notes/"[..]][..]),
            (&[b"a", b"b", b"a"], &[b"a", b"b", b"a"]),
            (&[b"--", b"-x"], &[b"-x"]),
            // The first `--` ends the options; a later one is a path.
            (&[b"a", b"--", b"-h", b"--"], &[b"a", b"-h", b"--"]),
            (
                &[b"caf\xe9", b"a b\r", b"x!#\\"],
                &[b"caf\xe9", b"a b\r", b"x!#\\"],
            ),
            (
                &[b"", b"../x", b"/abs", b"a:b"],
                &[b"", b"../x", b"/abs", b"a:b"],
            ),
        ] {
            assert_eq!(read(&words(given)), paths(expected), "{given:?}");
        }
    }

    #[test]
    fn a_help_request_anywhere_before_the_end_of_options_decides() {
        for request in [
            &[&b"-h"[..]][..],
            &[b"--help"],
            &[b"x", b"-h"],
            &[b"-f", b"--help"],
            &[b"a*", b"-h"],
            &[b"x", b"-h", b"--", b"y"],
        ] {
            assert_eq!(read(&words(request)), Asked::Help, "{request:?}");
        }
    }

    #[test]
    fn an_option_no_path_and_a_word_that_is_no_literal_path_are_misuses_naming_it() {
        let word = |word: &'static [u8]| OsStr::from_bytes(word);
        for (misuse, fault) in [
            (&[][..], Fault::NoPath),
            (&[&b"--"[..]], Fault::NoPath),
            (&[b"-f", b"x"], Fault::Option(word(b"-f"))),
            (&[b"x", b"--force"], Fault::Option(word(b"--force"))),
            (&[b"-"], Fault::Option(word(b"-"))),
            (&[b"a*"], Fault::Pattern(word(b"a*"))),
            (&[b"a?"], Fault::Pattern(word(b"a?"))),
            (&[b"a[1]"], Fault::Pattern(word(b"a[1]"))),
            (&[b"--", b"x*"], Fault::Pattern(word(b"x*"))),
            (&[b":/x"], Fault::Magic(word(b":/x"))),
            (&[b":x"], Fault::Magic(word(b":x"))),
            (&[b"x", b"--", b":"], Fault::Magic(word(b":"))),
            // A newline is found before anything that would name the word.
            (&[b"a\nb"], Fault::Newline),
            (&[b"-\n"], Fault::Newline),
            (&[b"*\n"], Fault::Newline),
        ] {
            assert_eq!(read(&words(misuse)), Asked::Misused(fault), "{misuse:?}");
        }
    }

    #[test]
    fn what_still_hides_a_path_given_to_unhide_is_what_its_lines_leave() {
        let paths =
            |paths: &[&[u8]]| -> Vec<Vec<u8>> { paths.iter().map(|p| p.to_vec()).collect() };
        let listed = paths(&[b"notes", b"notes/sub", b"a", b"a/b/c", b".env"]);
        let tracked: BTreeSet<Vec<u8>> = paths(&[b".env", b"docs/x.md", b"notes/sub/t.md"])
            .into_iter()
            .collect();
        let given = paths(&[b"notes/sub", b"a", b"a/b/c/d", b".env", b"docs", b"typo"]);
        for (path, expected) in [
            (&b".env"[..], Stays::Tracked),
            // A listed path above outranks the tracked files below.
            (b"notes/sub", Stays::Below(b"notes".to_vec())),
            // `a` goes with this `unhide`; `a/b/c` stays.
            (b"a/b/c/d", Stays::Below(b"a/b/c".to_vec())),
            (b"docs", Stays::TrackedBelow),
            (b"a", Stays::Nothing),
            (b"typo", Stays::Nothing),
            // Below means below a whole component.
            (b"doc", Stays::Nothing),
        ] {
            let found = stays(&listed, &tracked, &given, path);
            assert_eq!(found, expected, "{}", path.escape_ascii());
        }
    }

    #[test]
    fn an_unhide_hint_says_what_became_of_the_line_and_never_that_a_path_is_visible() {
        let removed = |stays| unhidden_line(b"p", true, &stays);
        assert_eq!(
            removed(Stays::Nothing),
            b"p is no longer listed in .gitdupe"
        );
        assert_eq!(
            unhidden_line(b"p", false, &Stays::Nothing),
            b"p is not listed in .gitdupe; there is no line to remove"
        );
        let lines = [
            removed(Stays::Tracked),
            removed(Stays::Below(b"q".to_vec())),
            removed(Stays::TrackedBelow),
            unhidden_line(b"p", false, &Stays::Tracked),
        ];
        assert!(lines[0].ends_with(b"it stays hidden, because the private repository tracks it"));
        assert!(lines[1].ends_with(b"it stays hidden below q, which .gitdupe lists"));
        assert!(
            lines[2].ends_with(b"files below it that the private repository tracks stay hidden")
        );
        for line in &lines {
            assert!(
                !line.windows(7).any(|part| part == b"visible"),
                "{}",
                line.escape_ascii()
            );
        }
    }

    #[test]
    fn only_hide_sends_the_developer_to_add_and_never_for_a_tracked_path() {
        let holds = |line: &[u8], part: &[u8]| line.windows(part.len()).any(|at| at == part);
        let hint = |hiding, tracked, absent| newly_hidden(b"p", tracked, hiding, || absent);
        let present = hint(Hiding::Hide, false, false);
        let absent = hint(Hiding::Hide, false, true);
        let by_add = hint(Hiding::Add, false, false);
        let tracked = hint(Hiding::Hide, true, false);
        for line in [&present, &absent, &by_add, &tracked] {
            assert!(line.starts_with(b"p "), "{}", line.escape_ascii());
            assert!(
                holds(line, b"'git dupe unhide -- p'"),
                "{}",
                line.escape_ascii()
            );
        }
        assert!(holds(
            &present,
            b"run from the root, 'git dupe add -- p' versions it"
        ));
        assert!(!holds(&present, b"nothing stands"));
        assert!(holds(&absent, b"nothing stands there yet"));
        assert!(holds(
            &absent,
            b"run from the root, 'git dupe add -- p' versions it"
        ));
        assert!(!holds(&by_add, b"git dupe add"));
        assert!(!holds(&tracked, b"git dupe add"));
        assert!(!holds(&tracked, b"now hidden"));
    }

    #[test]
    fn the_route_to_private_names_the_deletion_and_then_add() {
        let holds = |line: &[u8], part: &[u8]| line.windows(part.len()).any(|at| at == part);
        let general = route_to_private(None);
        let whole = route_to_private(Some(b"-p"));
        for line in [&general, &whole] {
            let at = |part: &[u8]| line.windows(part.len()).position(|at| at == part);
            let (Some(rm), Some(add)) = (at(b"'git rm --cached"), at(b"'git dupe add")) else {
                panic!("{}", line.escape_ascii());
            };
            assert!(rm < add, "{}", line.escape_ascii());
            assert!(holds(line, b"a deletion from the project"));
            assert!(holds(line, b"every other clone"));
            assert!(!holds(line, b"--cached' first makes"));
        }
        assert!(general.starts_with(b"such a path becomes private one file at a time: "));
        assert!(whole.starts_with(b"run from the root, 'git rm --cached -- -p' first, "));
        assert!(whole.ends_with(
            b"then 'git dupe add -- -p' makes it private, \
              or 'git dupe add -f -- -p' where the project ignores it"
        ));
        assert!(
            general.ends_with(b"then 'git dupe add' of it, with -f where the project ignores it")
        );
    }

    #[test]
    fn a_fault_names_the_word_as_typed_on_one_line() {
        let word = OsStr::from_bytes(b"caf\xe9 *");
        for fault in [
            Fault::Option(word),
            Fault::Pattern(word),
            Fault::Magic(word),
            Fault::Root(word),
            Fault::Outside(word),
        ] {
            assert!(fault.text().starts_with(b"'caf\xe9 *' "), "{fault:?}");
            assert!(!fault.text().contains(&b'\n'), "{fault:?}");
        }
        for fault in [Fault::NoPath, Fault::Newline] {
            assert!(!fault.text().contains(&b'\n'), "{fault:?}");
        }
    }
}
