//! The locate run and the workspace facts.
//!
//! One `git rev-parse`, in the environment git-dupe received and from its directory, is
//! the only discovery: `-C`, `--git-dir`, and `--work-tree` before `dupe` name the
//! repository as Git read them. Its answer is read as bytes, never as text, and
//! its failure is Git's own answer: which failure it was is never read from the message.
//! For a command that attaches, the same run also asks for the superproject, whose
//! answer marks a submodule checkout (G4).
//!
//! `rev-parse` prints its answers in turn and stops at the first it cannot give (S12).
//! A failed locate that printed nothing was made outside any repository; one that
//! printed the Git directory and the common Git directory, in a bare repository or inside
//! a Git directory. That is all that is read of a failure, and only a run the front
//! makes without a repository uses it (`Unlocated`).
//!
//! The facts a destination is compared from (`Facts`, `Holds/G18`) come from the same
//! answers, and one more: the user's directory where it may lie outside the root, asked
//! of the process when a guard needs it: located, wherever the locate printed no prefix,
//! which is outside the root or at the root itself, where reading a destination from it
//! is reading it from the root again. Where the locate failed after naming a Git
//! directory, there is no root: the common Git directory's parent stands for it, which
//! is the root wherever a workspace is attached (`Holds/G1`), and the user's directory,
//! inside a Git directory, is always read too. The facts also hold the value of `HOME`
//! git-dupe received, which every run passes through and from which Git reads a local
//! destination's leading `~` (S13).

use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::{Against, End, Failure, Run};

const LOCATE: [&str; 6] = [
    "rev-parse",
    "--path-format=absolute",
    "--git-dir",
    "--git-common-dir",
    "--show-toplevel",
    "--show-prefix",
];

/// Asked immediately before `--show-prefix`, so that the prefix stays the rest of the
/// answer. Outside a submodule checkout it prints nothing, not even an empty line; inside
/// one it prints the superproject's working tree, absolute.
const SUPERPROJECT: &str = "--show-superproject-working-tree";

/// What the locate run established about where git-dupe stands.
pub struct Workspace {
    git_directory: PathBuf,
    common_directory: PathBuf,
    root: PathBuf,
    prefix: Vec<u8>,
    attached: bool,
    submodule_checkout: bool,
}

/// Why there is no workspace to work in.
pub enum NotLocated {
    /// Git refused: outside any repository, or where there is no working tree. Its
    /// message and its end are the command's, but for what the front answers without a
    /// repository, made where `unlocated` says; `None` when what Git printed before it
    /// failed is neither of the two answers a failure gives.
    Refused {
        message: Vec<u8>,
        end: End,
        unlocated: Option<Unlocated>,
    },
    /// Git answered with something other than the four answers asked for.
    Unreadable,
    NotStarted(Failure),
}

/// Locates the workspace. `for_attaching` also asks whether it is a submodule checkout,
/// which only a command that attaches needs to know.
pub fn locate(for_attaching: bool) -> Result<Workspace, NotLocated> {
    let run = Run::public(words(for_attaching))
        .capture_output()
        .capture_errors()
        .start()
        .map_err(NotLocated::NotStarted)?;
    if run.end != End::Code(0) {
        return Err(NotLocated::Refused {
            unlocated: Unlocated::read(&run.stdout),
            message: run.stderr,
            end: run.end,
        });
    }
    Workspace::read(&run.stdout, for_attaching).ok_or(NotLocated::Unreadable)
}

fn words(for_attaching: bool) -> Vec<&'static str> {
    let (before_prefix, prefix) = LOCATE.split_at(LOCATE.len() - 1);
    let superproject = for_attaching.then_some(SUPERPROJECT);
    before_prefix
        .iter()
        .copied()
        .chain(superproject)
        .chain(prefix.iter().copied())
        .collect()
}

impl Workspace {
    /// Reads the four answers: the Git directory, the common Git directory, and the root,
    /// absolute and one line each, then the prefix, relative and ending in `/`, or empty
    /// at the root.
    /// No working tree or Git directory of the project has a newline in its path, but a
    /// directory below the root may: the prefix is the rest of the answer.
    /// With `superproject` asked, a submodule checkout's answer has one more line before
    /// the prefix, the superproject's absolute path; a prefix never begins with `/`, so a
    /// line that does is that answer.
    /// An answer of any other shape is not the one asked for. A newline in one of the
    /// three paths, which git-dupe does not support, usually gives one; where it does
    /// not, the paths read here are not the real ones.
    fn read(answer: &[u8], superproject: bool) -> Option<Workspace> {
        let mut lines = answer.strip_suffix(b"\n")?.splitn(4, |&b| b == b'\n');
        let (Some(git_directory), Some(common_directory), Some(root), Some(mut prefix)) =
            (lines.next(), lines.next(), lines.next(), lines.next())
        else {
            return None;
        };
        let absolute = |path: &[u8]| path.starts_with(b"/");
        let submodule_checkout = superproject && absolute(prefix);
        if submodule_checkout {
            let (_, after) = prefix.split_at(prefix.iter().position(|&b| b == b'\n')?);
            prefix = &after[1..];
        }
        let relative = prefix.is_empty() || (!absolute(prefix) && prefix.ends_with(b"/"));
        if !(absolute(git_directory) && absolute(common_directory) && absolute(root) && relative) {
            return None;
        }
        let mut workspace = Workspace {
            git_directory: PathBuf::from(OsStr::from_bytes(git_directory)),
            common_directory: PathBuf::from(OsStr::from_bytes(common_directory)),
            root: PathBuf::from(OsStr::from_bytes(root)),
            prefix: prefix.to_vec(),
            attached: false,
            submodule_checkout,
        };
        workspace.attached = workspace.attached_now();
        Some(workspace)
    }

    /// Whether the workspace was attached when the command began.
    pub fn attached(&self) -> bool {
        self.attached
    }

    /// Whether the workspace is attached as things stand now, asked again after a
    /// handler that can attach it: the private Git directory is a directory while the
    /// Git directory is the common one. Its existence alone would make a linked worktree
    /// of an attached workspace attached.
    pub fn attached_now(&self) -> bool {
        !self.linked() && self.private_directory().is_dir()
    }

    /// The root: the working tree's top directory, absolute.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The user's directory relative to the root, as Git printed it: ending in `/`, or
    /// empty at the root. Operands are read relative to it.
    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }

    /// The common Git directory, absolute: the public repository's `.git`, which holds
    /// `info/exclude` and the private repository.
    pub fn common_directory(&self) -> &Path {
        &self.common_directory
    }

    /// Where the private repository lives: `dupe` in the common Git directory.
    pub fn private_directory(&self) -> PathBuf {
        private_directory(&self.common_directory)
    }

    /// Where a help request is run where no workspace is attached: `dupe` in the Git
    /// directory. Outside a linked worktree that is the private repository's path; in
    /// one it is a path nothing creates, so that private history is reached from the
    /// main working tree only and a word Git reads as a value fails as outside a
    /// repository (`Holds/G24`).
    pub fn help_directory(&self) -> PathBuf {
        private_directory(self.git_directory())
    }

    /// The Git directory, absolute, as the locate run printed it: the common Git
    /// directory in the main worktree, `<common Git directory>/worktrees/<name>` in a
    /// linked one (S17).
    pub fn git_directory(&self) -> &Path {
        &self.git_directory
    }

    /// In a linked worktree, its name: the last component of the Git directory, as bytes
    /// (E9, S17). The main worktree has none. Neither the root's name nor any other Git
    /// run decides it, so that a worktree moved with `git worktree move` keeps it.
    pub fn worktree_name(&self) -> Option<&OsStr> {
        self.linked()
            .then(|| self.git_directory().file_name())
            .flatten()
    }

    /// Whether the locate run, made for a command that attaches, found a superproject:
    /// this working tree is a submodule checkout. A locate for any other command does
    /// not ask, and this is false there.
    pub fn submodule_checkout(&self) -> bool {
        self.submodule_checkout
    }

    /// The facts a destination is compared from: the root, the common Git directory, the
    /// user's directory where the locate printed no prefix, and `HOME` (`Facts`). Below the
    /// root that directory is none; a directory that cannot be had is none, because Git
    /// could not read a destination from it either.
    pub fn facts(&self) -> Facts {
        Facts {
            root: self.root.clone(),
            common_directory: self.common_directory.clone(),
            outside: self
                .prefix
                .is_empty()
                .then(env::current_dir)
                .and_then(Result::ok),
            home: env::var_os("HOME"),
        }
    }

    /// Whether this is a linked worktree: its Git directory is not the common one.
    pub fn linked(&self) -> bool {
        self.git_directory != self.common_directory
    }

    /// In a linked worktree, the main working tree where Git's `worktree list` names it:
    /// the common Git directory's parent, when that directory is named `.git`. Elsewhere,
    /// as for a Git directory made with `--separate-git-dir` or a submodule's, that parent
    /// is no working tree, and Git itself does not know the main one.
    pub fn main_working_tree_of_linked(&self) -> Option<&Path> {
        if !self.linked() || self.common_directory.file_name() != Some(OsStr::new(".git")) {
            return None;
        }
        self.common_directory.parent()
    }
}

#[cfg(test)]
impl Workspace {
    /// The main working tree at `root`, absolute, as the locate run names it from the
    /// root: for unit checks of what a part does on disk there.
    pub fn at_root(root: &Path) -> Workspace {
        let root = root.as_os_str().as_bytes();
        let answer = [root, b"/.git\n", root, b"/.git\n", root, b"\n\n"].concat();
        Workspace::read(&answer, false).expect("an absolute root without a newline")
    }

    /// The linked worktree `name` of the common Git directory `common`, at `root`, all
    /// absolute, as the locate run names it from that root.
    pub fn linked_at(common: &Path, name: &[u8], root: &Path) -> Workspace {
        let common = common.as_os_str().as_bytes();
        let root = root.as_os_str().as_bytes();
        let answer = [
            common,
            b"/worktrees/",
            name,
            b"\n",
            common,
            b"\n",
            root,
            b"\n\n",
        ]
        .concat();
        Workspace::read(&answer, false).expect("absolute paths without a newline")
    }
}

fn private_directory(common_directory: &Path) -> PathBuf {
    common_directory.join("dupe")
}

/// Where a destination is compared from (`Holds/G18`), all absolute: the root a relative
/// destination is read from, the common Git directory that holds `worktrees/`, and the
/// user's directory where a relative destination is also read from when it lies outside
/// the root (`outside`; it may be the root itself, which reads the same). And `home`, the
/// value of `HOME` as received, which may be anything: Git reads a destination's leading
/// `~` from it, and none when it is unset (S13).
pub struct Facts {
    root: PathBuf,
    common_directory: PathBuf,
    outside: Option<PathBuf>,
    home: Option<OsString>,
}

impl Facts {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn common_directory(&self) -> &Path {
        &self.common_directory
    }

    pub fn outside(&self) -> Option<&Path> {
        self.outside.as_deref()
    }

    pub fn home(&self) -> Option<&OsStr> {
        self.home.as_deref()
    }
}

/// Where a run is made that needs no repository, after a locate that failed
/// (`Composition/Runner`).
pub enum Unlocated {
    /// The locate printed nothing: outside any repository. Such a run gets the environment
    /// received.
    Outside,
    /// The locate printed the Git directory and the common Git directory: a bare
    /// repository, or a directory inside a Git directory. Such a run is private, its Git
    /// directory `dupe` in that Git directory, with no working tree, so that it never
    /// selects the repository found there, whose own aliases could write (G5). Where the
    /// two differ, inside a linked worktree's Git directory, that path never exists, so
    /// that the main working tree's private history is not reached from there either
    /// (`Holds/G24`). The common Git directory is kept as printed, for the facts.
    InGitDirectory {
        git_directory: PathBuf,
        common_directory: PathBuf,
    },
}

impl Unlocated {
    /// What a failed locate printed before failing: nothing, or two absolute lines, the
    /// Git directory and the common Git directory. Any other answer is neither.
    fn read(printed: &[u8]) -> Option<Unlocated> {
        if printed.is_empty() {
            return Some(Unlocated::Outside);
        }
        let mut lines = printed.strip_suffix(b"\n")?.split(|&b| b == b'\n');
        let (Some(git_directory), Some(common_directory), None) =
            (lines.next(), lines.next(), lines.next())
        else {
            return None;
        };
        if !(git_directory.starts_with(b"/") && common_directory.starts_with(b"/")) {
            return None;
        }
        Some(Unlocated::InGitDirectory {
            git_directory: private_directory(Path::new(OsStr::from_bytes(git_directory))),
            common_directory: PathBuf::from(OsStr::from_bytes(common_directory)),
        })
    }

    /// The repository such a run is made against.
    pub fn against(&self) -> Against<'_> {
        match self {
            Unlocated::Outside => Against::Public,
            Unlocated::InGitDirectory { git_directory, .. } => {
                Against::PrivateWithoutWorkTree { git_directory }
            }
        }
    }

    /// The facts a destination is compared from, inside a Git directory: its common Git
    /// directory, that directory's parent as the root, the user's directory always, and
    /// `HOME`. Outside any repository there is no public place, and none.
    pub fn facts(&self) -> Option<Facts> {
        let Unlocated::InGitDirectory {
            common_directory, ..
        } = self
        else {
            return None;
        };
        Some(Facts {
            root: common_directory
                .parent()
                .unwrap_or(common_directory)
                .to_path_buf(),
            common_directory: common_directory.clone(),
            outside: env::current_dir().ok(),
            home: env::var_os("HOME"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(answer: &[u8]) -> Workspace {
        Workspace::read(answer, false).expect("four answers")
    }

    #[test]
    fn at_the_root_the_prefix_line_is_empty() {
        let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\n\n");
        assert_eq!(workspace.git_directory, Path::new("/w/repo/.git"));
        assert_eq!(workspace.common_directory(), Path::new("/w/repo/.git"));
        assert_eq!(workspace.root(), Path::new("/w/repo"));
        assert_eq!(
            workspace.private_directory(),
            Path::new("/w/repo/.git/dupe")
        );
        assert_eq!(workspace.prefix(), b"");
        assert_eq!(workspace.main_working_tree_of_linked(), None);
        assert_eq!(workspace.help_directory(), workspace.private_directory());
        assert!(!workspace.linked());
        assert!(!workspace.attached());
    }

    #[test]
    fn below_the_root_the_answer_names_the_same_repository() {
        let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/dir/\n");
        assert_eq!(workspace.git_directory, Path::new("/w/repo/.git"));
        assert_eq!(workspace.root(), Path::new("/w/repo"));
        assert_eq!(workspace.prefix(), b"sub/dir/");
        assert_eq!(workspace.main_working_tree_of_linked(), None);
    }

    #[test]
    fn in_a_linked_worktree_the_main_working_tree_is_the_common_directorys_parent() {
        let workspace = read(b"/w/repo/.git/worktrees/linked\n/w/repo/.git\n/w/linked\nsub/\n");
        assert_eq!(
            workspace.git_directory,
            Path::new("/w/repo/.git/worktrees/linked")
        );
        assert_eq!(workspace.common_directory, Path::new("/w/repo/.git"));
        assert!(workspace.linked());
        assert_eq!(
            workspace.main_working_tree_of_linked(),
            Some(Path::new("/w/repo"))
        );
        // A help request there never reaches the main working tree's private repository.
        assert_eq!(
            workspace.help_directory(),
            Path::new("/w/repo/.git/worktrees/linked/dupe")
        );
    }

    #[test]
    fn where_the_common_directory_is_not_named_dot_git_no_main_working_tree_is_known() {
        // A repository made with `--separate-git-dir`, and a submodule's.
        for answer in [
            &b"/w/s/admin/worktrees/linked\n/w/s/admin\n/w/s/linked\n\n"[..],
            b"/w/super/.git/modules/sub/worktrees/l\n/w/super/.git/modules/sub\n/w/l\n\n",
        ] {
            let workspace = read(answer);
            assert!(workspace.linked(), "{}", answer.escape_ascii());
            assert_eq!(
                workspace.main_working_tree_of_linked(),
                None,
                "{}",
                answer.escape_ascii()
            );
        }
    }

    #[test]
    fn the_worktree_is_named_by_its_git_directory_alone() {
        // The main worktree has no name, its Git directory the common one.
        let main = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/\n");
        assert_eq!(main.git_directory(), Path::new("/w/repo/.git"));
        assert_eq!(main.worktree_name(), None);
        // A linked worktree's name is its Git directory's last component, whatever its
        // root is called: an ordinary repository's, a bare one's, and one whose name is
        // not UTF-8.
        for (answer, git_directory, name) in [
            (
                &b"/w/repo/.git/worktrees/agent\n/w/repo/.git\n/w/project-agent\n\n"[..],
                &b"/w/repo/.git/worktrees/agent"[..],
                &b"agent"[..],
            ),
            (
                b"/w/bare.git/worktrees/one\n/w/bare.git\n/w/elsewhere/two\nsub/\n",
                b"/w/bare.git/worktrees/one",
                b"one",
            ),
            (
                b"/w/repo/.git/worktrees/caf\xe9\xff\n/w/repo/.git\n/w/moved\n\n",
                b"/w/repo/.git/worktrees/caf\xe9\xff",
                b"caf\xe9\xff",
            ),
        ] {
            let workspace = read(answer);
            assert_eq!(
                workspace.git_directory(),
                Path::new(OsStr::from_bytes(git_directory)),
                "{}",
                answer.escape_ascii()
            );
            assert_eq!(
                workspace.worktree_name(),
                Some(OsStr::from_bytes(name)),
                "{}",
                answer.escape_ascii()
            );
            // Nothing else moves with the name: the private repository is still the
            // common Git directory's, and a linked worktree is still not attached.
            assert_eq!(
                workspace.private_directory(),
                private_directory(workspace.common_directory())
            );
            assert!(workspace.linked());
            assert!(!workspace.attached());
            assert!(!workspace.attached_now());
        }
    }

    #[test]
    fn the_private_directory_is_dupe_in_the_common_directory() {
        assert_eq!(
            private_directory(Path::new("/w/repo/.git")),
            Path::new("/w/repo/.git/dupe")
        );
    }

    #[test]
    fn paths_are_bytes() {
        let workspace = read(b"/w/caf\xe9/.git/worktrees/x\n/w/caf\xe9/.git\n/w/x\xff\n\n");
        assert_eq!(
            workspace.main_working_tree_of_linked(),
            Some(Path::new(OsStr::from_bytes(b"/w/caf\xe9")))
        );
        assert_eq!(workspace.root(), Path::new(OsStr::from_bytes(b"/w/x\xff")));
    }

    #[test]
    fn a_prefix_holding_a_newline_is_the_rest_of_the_answer() {
        let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nnew\nline\n/sub/\n");
        assert_eq!(workspace.prefix(), b"new\nline\n/sub/");
        assert_eq!(workspace.git_directory, Path::new("/w/repo/.git"));
        assert_eq!(workspace.common_directory, Path::new("/w/repo/.git"));
        let workspace = read(b"/w/repo/.git/worktrees/l\n/w/repo/.git\n/w/l\nnew\nline\n/\n");
        assert_eq!(
            workspace.main_working_tree_of_linked(),
            Some(Path::new("/w/repo"))
        );
    }

    #[test]
    fn an_answer_of_another_shape_is_not_read() {
        for answer in [
            &b""[..],
            b"/w/repo/.git\n/w/repo/.git\n",
            b"/w/repo/.git\n/w/repo/.git\n/w/repo\n",
            b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/",
            b"repo/.git\n/w/repo/.git\n/w/repo\n\n",
            b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub\n",
            // A newline in one of the three paths, outside the envelope, as Git prints
            // it: at the root, and below it.
            b"/w/re\npo/.git\n/w/re\npo/.git\n/w/re\npo\n\n",
            b"/w/re\n/po/.git\n/w/re\n/po/.git\n/w/re\n/po\n\n",
            b"/w/re\n/po/.git\n/w/re\n/po/.git\n/w/re\n/po\nsub/\n",
            b"/w/repo/.git/worktrees/l\n/w/repo/.git\n/w/li\nnked\n\n",
        ] {
            for superproject in [false, true] {
                assert!(
                    Workspace::read(answer, superproject).is_none(),
                    "{}",
                    answer.escape_ascii()
                );
            }
        }
    }

    #[test]
    fn a_failed_locate_is_read_from_what_it_printed_before_failing() {
        assert!(matches!(Unlocated::read(b""), Some(Unlocated::Outside)));
        assert!(Unlocated::Outside.facts().is_none());
        // Inside a workspace's `.git` the two answers are the same directory; in a linked
        // worktree's Git directory the common one is the main repository's, whose private
        // repository is never the one used there.
        for (printed, private) in [
            (
                &b"/w/repo/.git\n/w/repo/.git\n"[..],
                &b"/w/repo/.git/dupe"[..],
            ),
            (b"/w/bare.git\n/w/bare.git\n", b"/w/bare.git/dupe"),
            (
                b"/w/repo/.git/worktrees/l\n/w/repo/.git\n",
                b"/w/repo/.git/worktrees/l/dupe",
            ),
            (b"/w/caf\xe9.git\n/w/caf\xe9.git\n", b"/w/caf\xe9.git/dupe"),
        ] {
            let Some(unlocated) = Unlocated::read(printed) else {
                panic!("{}", printed.escape_ascii());
            };
            let Unlocated::InGitDirectory {
                git_directory,
                common_directory,
            } = &unlocated
            else {
                panic!("{}", printed.escape_ascii());
            };
            assert_eq!(
                git_directory,
                Path::new(OsStr::from_bytes(private)),
                "{}",
                printed.escape_ascii()
            );
            // The facts keep the common Git directory the locate printed, and its parent
            // stands for the root.
            let facts = unlocated.facts().expect("facts inside a Git directory");
            assert_eq!(facts.common_directory(), common_directory);
            assert_eq!(Some(facts.root()), common_directory.parent());
            assert!(facts.outside().is_some());
        }
        for printed in [
            &b"/w/repo/.git\n"[..],
            b"/w/repo/.git\n/w/repo/.git",
            b"/w/repo/.git\n/w/repo/.git\n/w/repo\n",
            b".git\n.git\n",
            b"\n\n",
        ] {
            assert!(
                Unlocated::read(printed).is_none(),
                "{}",
                printed.escape_ascii()
            );
        }
    }

    #[test]
    fn the_superproject_is_asked_immediately_before_the_prefix() {
        assert_eq!(words(false), LOCATE);
        assert_eq!(
            words(true),
            [
                "rev-parse",
                "--path-format=absolute",
                "--git-dir",
                "--git-common-dir",
                "--show-toplevel",
                "--show-superproject-working-tree",
                "--show-prefix",
            ]
        );
    }

    #[test]
    fn asked_for_the_superproject_outside_a_submodule_the_answer_keeps_four_lines() {
        for answer in [
            &b"/w/repo/.git\n/w/repo/.git\n/w/repo\n\n"[..],
            b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/dir/\n",
            b"/w/repo/.git\n/w/repo/.git\n/w/repo\nnew\nline\n/\n",
        ] {
            let workspace = Workspace::read(answer, true).expect("four answers");
            assert!(!workspace.submodule_checkout(), "{}", answer.escape_ascii());
            assert_eq!(workspace.root(), Path::new("/w/repo"));
        }
    }

    #[test]
    fn a_superproject_line_before_the_prefix_marks_a_submodule_checkout() {
        for (answer, root) in [
            (
                &b"/w/super/.git/modules/sub\n/w/super/.git/modules/sub\n/w/super/sub\n/w/super\n\n"
                    [..],
                "/w/super/sub",
            ),
            (
                b"/w/s/.git/modules/m\n/w/s/.git/modules/m\n/w/s/m\n/w/s\nnew\nline\n/x/\n",
                "/w/s/m",
            ),
        ] {
            let workspace = Workspace::read(answer, true).expect("five answers");
            assert!(workspace.submodule_checkout(), "{}", answer.escape_ascii());
            assert_eq!(workspace.root(), Path::new(root));
            assert!(!workspace.prefix().starts_with(b"/"));
        }
        // Not asked, the same answer is of another shape; and a superproject line must
        // be followed by a prefix.
        let submodule =
            b"/w/super/.git/modules/sub\n/w/super/.git/modules/sub\n/w/super/sub\n/w/super\n\n";
        assert!(Workspace::read(submodule, false).is_none());
        assert!(Workspace::read(b"/w/r/.git\n/w/r/.git\n/w/r\n/w/super\n", true).is_none());
        assert!(Workspace::read(b"/w/r/.git\n/w/r/.git\n/w/r\n/w/s\n/x/\n", true).is_none());
    }
}
