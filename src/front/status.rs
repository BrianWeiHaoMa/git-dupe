//! `status`: its table, the resolution of its paths, its handler, and its line (G12,
//! `Holds/G12`).
//!
//! What the code below cannot show:
//!
//! - The run captures no stream: its standard output, standard error, color, pager, and
//!   exit status are Git's (F9, G25), the diff `-v` adds included. Its command word is the
//!   user's, so it carries `-c help.autocorrect=0` as every such run does (`runner`).
//! - Every pathspec of the run is git-dupe's (`Composition/Guards`), so the run is marked
//!   as having its own paths and no pathspec setting before `dupe` reaches it (G10, G19).
//!   What confines it is decided in `guards::status` over the listings taken here.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::table::{Entry, Fault, Read, Value};
use crate::guards::operand::{self, Cleaned};
use crate::guards::{pathspec, status as confinement};
use crate::keeper::{self, GITDUPE};
use crate::runner::locate::Workspace;
use crate::runner::{Failure, Run};

const UNTRACKED_FILES: Entry = Entry::both(b'u', "untracked-files", Value::Attached);

const IGNORED: Entry = Entry::long("ignored", Value::Attached);

/// P1's table for `status`: every value here is optional and attached.
pub const TABLE: [Entry; 17] = [
    Entry::both(b'v', "verbose", Value::None),
    Entry::both(b's', "short", Value::None),
    Entry::both(b'b', "branch", Value::None),
    Entry::long("show-stash", Value::None),
    Entry::long("ahead-behind", Value::None),
    Entry::long("no-ahead-behind", Value::None),
    Entry::long("porcelain", Value::Attached),
    Entry::long("long", Value::None),
    Entry::both(b'z', "null", Value::None),
    UNTRACKED_FILES,
    IGNORED,
    Entry::long("ignore-submodules", Value::Attached),
    Entry::long("column", Value::Attached),
    Entry::long("no-column", Value::None),
    Entry::long("renames", Value::None),
    Entry::long("no-renames", Value::None),
    Entry::both(b'M', "find-renames", Value::Attached),
];

/// The run of an empty scope: no walk enters `.git` and no index entry lies under it, so
/// Git prints its status confined to nothing (`Holds/G12`).
const NOTHING: &[u8] = b".git";

/// The paths the operands name, root-relative, the root as the empty path; `None` without
/// an operand. An operand that resolves outside the working tree is the fault.
pub fn resolve<'w>(
    workspace: &Workspace,
    operands: &[&'w OsStr],
) -> Result<Option<Vec<Vec<u8>>>, Fault<'w>> {
    if operands.is_empty() {
        return Ok(None);
    }
    operands
        .iter()
        .map(|&word| {
            match operand::resolve_rooted(word.as_bytes(), workspace.prefix(), workspace.root()) {
                Cleaned::Inside(path) => Ok(path),
                Cleaned::Root => Ok(Vec::new()),
                Cleaned::Outside(_) => Err(Fault::Outside(word)),
            }
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

/// The `status` handler: the hidden paths, the staged deletions, the scope, one public
/// listing under the scope and `.gitdupe`, then Git's status confined to the scope, and
/// the `hint:` while nothing but `.gitdupe` is hidden.
pub fn status(workspace: &Workspace, read: &Read, paths: Option<&[Vec<u8>]>) -> Outcome {
    let hidden = match keeper::hidden(workspace) {
        Ok(hidden) => hidden,
        Err(failed) => return outcome::failed(failed),
    };
    let deletions = match keeper::staged_deletions(workspace) {
        Ok(deletions) => deletions,
        Err(failed) => return outcome::failed(failed),
    };
    let region = &hidden.paths.region;
    let scope = confinement::scope(region, &deletions, paths);
    // `.gitdupe` keeps the query from being empty, which would list the whole index.
    let mut query = scope.clone();
    if scope
        .binary_search_by(|path| path.as_slice().cmp(GITDUPE))
        .is_err()
    {
        query.push(GITDUPE.to_vec());
    }
    let publicly_tracked = match keeper::publicly_tracked(workspace, &query) {
        Ok(tracked) => tracked,
        Err(failed) => return outcome::failed(failed),
    };
    let privately_tracked = &hidden.paths.privately_tracked;
    let exclusions =
        confinement::exclusions(&scope, &publicly_tracked, privately_tracked, &deletions);

    let confined: Vec<OsString> = if scope.is_empty() {
        vec![pathspec::top_literal(NOTHING)]
    } else {
        scope
            .iter()
            .map(|path| pathspec::top_literal(path))
            .collect()
    };
    let carried = confined.len() + exclusions.len();
    let mut words: Vec<OsString> = vec!["status".into()];
    words.extend(read.options().map(OsStr::to_owned));
    if !read.spells(&UNTRACKED_FILES) {
        words.push("--untracked-files=normal".into());
    }
    if !read.spells(&IGNORED) {
        words.push("--ignored".into());
    }
    words.push("--".into());
    words.extend(confined);
    words.extend(exclusions.iter().map(confinement::Exclusion::pathspec));

    let private = workspace.private_directory();
    match Run::private(&private, workspace.root(), words)
        .own_paths()
        .users_command()
        .start()
    {
        Ok(finished) => {
            if hidden.paths.only_gitdupe() {
                lines::write(
                    Level::Hint,
                    b"nothing is private yet; 'git dupe add <path>' makes a path private",
                );
            }
            Outcome::Git(finished.end)
        }
        Err(Failure::TooLong) => outcome::list_too_long(carried),
        Err(failure) => outcome::not_started(failure),
    }
}
