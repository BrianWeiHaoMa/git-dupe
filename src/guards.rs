//! Guards: operands and the lexical cleaning of a path, what git-dupe writes for Git to
//! read as a pathspec, and the refusals decided before anything is written or run.
//!
//! What the listing below cannot show:
//!
//! - A path is cleaned by `operand` alone, lexically and never through the filesystem
//!   (R9): a `.gitdupe` line and a typed operand are compared in the same form.
//! - Every pathspec git-dupe writes is made by `pathspec`, and no other code writes one:
//!   a path git-dupe hands Git is anchored at the root and read as that path whatever it
//!   holds and whatever directory the run is made from: literal, or, for `status`'s
//!   exclusion of a path alone, a glob escaped to match it (S4). A run carrying one is
//!   marked as having git-dupe's own paths, so that no pathspec variable of a global
//!   option before `dupe` reaches it (`runner::environment`).
//! - A refusal, and what confines a run, is decided here over listings the caller took,
//!   and returned: this part runs nothing and writes nothing, and the caller writes only
//!   after it has asked. `scope` is the one rule for the hidden paths under a path.
//!   `clean`'s confinement is no pathspec but `-e` patterns, written by `clean` over the
//!   paths the caller found and the keeper's rule form, which the caller hands in.
//! - The public places and the form of a destination are here (G18): `places` builds the
//!   places from what the caller took and answers whether a word names one, by the one
//!   form `destination` gives a word, a configured URL, or a default remote. The
//!   canonical path of a local destination is its real path, which is the only reading of
//!   the filesystem in this part; `clone` and the transfer commands ask the same
//!   question of the same places. `transfer` reads what else `push`, `pull`, and `fetch`
//!   are compared by: whether their words give a repository, and, from the private
//!   configuration the caller took, the URLs of its remotes and the default remote.
//! - A name a line must give that may hold a newline, a word the user typed or a file Git
//!   reads rules from, is written by `quoted`, as Git quotes a path, so that the front's
//!   lines and the keeper's warnings stay one line each (F8); a path a warning offers to a
//!   command it names, by `shell_word`, so that the command as shown takes it whole.

pub mod add;
pub mod clean;
pub mod destination;
pub mod hide;
pub mod operand;
pub mod pathspec;
pub mod places;
pub mod quoted;
pub mod scope;
pub mod stash;
pub mod status;
pub mod transfer;
