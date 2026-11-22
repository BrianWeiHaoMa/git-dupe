//! The public places and the question asked of a word (G18, `Holds/G18`): does it name
//! the project's repository, one of its working trees, or a remote of it, and which.
//!
//! What the code below cannot show:
//!
//! - The places are built once per command from what the caller took: every URL the
//!   public configuration gives a remote, with the remote's name; every working tree root
//!   `git worktree list` names; and the Git directories, the common one and each directly
//!   under its `worktrees/`. Each root's `.git` entry is `<root>/.git`.
//! - A word names a place when one of its forms (`destination`) equals a form of a public
//!   URL, or when one of its local forms equals the canonical path of a root, a `.git`
//!   entry, or a Git directory. Nothing is compared by prefix: a path that merely begins
//!   like a place names nothing.
//! - A word is asked whole and, when it holds `=`, as the part after its first `=`; a URL
//!   the private configuration gives a remote, and a default remote's value, are asked
//!   whole alone. The same reading of a relative path, and of one beginning with `~`,
//!   serves them and the public URLs, so that a URL and a word that Git would open at one
//!   path compare equal.
//! - The same places say which other worktree's private repository a URL names, for the
//!   hint after a push to one (`Holds/G18`): the URL's local forms against the canonical
//!   path of `dupe` in each Git directory other than the one the command runs in. The main worktree's is the common Git directory's, and a linked
//!   worktree's is named for its Git directory's last component, as G27's warnings name it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::destination::{self, Form, ReadFrom};

/// A public place, as a refusal names it: what the configuration or Git gave, as bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A URL the public configuration gives the remote `name`, as configured.
    Remote { name: Vec<u8>, url: Vec<u8> },
    /// A working tree's root, as `git worktree list` names it.
    WorkingTree(Vec<u8>),
    /// A working tree's `.git` entry.
    GitEntry(Vec<u8>),
    /// The common Git directory, or a linked worktree's Git directory.
    GitDirectory(Vec<u8>),
}

/// A worktree of the project, as git-dupe's lines name it (G27).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Worktree {
    Main,
    /// The linked worktree of this name, as bytes.
    Linked(Vec<u8>),
}

impl Worktree {
    /// `the main worktree`, or `worktree <name>`, the name's bytes as they stand.
    pub fn named(&self) -> Vec<u8> {
        match self {
            Worktree::Main => b"the main worktree".to_vec(),
            Worktree::Linked(name) => [&b"worktree "[..], name].concat(),
        }
    }
}

/// The public places of one command, and where a relative path is read from.
pub struct Places {
    root: PathBuf,
    outside: Option<PathBuf>,
    home: Option<Vec<u8>>,
    /// Every form of every public URL, with the first URL that has it.
    urls: BTreeMap<Form, Place>,
    /// The canonical path of every root, `.git` entry, and Git directory.
    paths: BTreeMap<Vec<u8>, Place>,
    /// Each Git directory as given, with the worktree it is the Git directory of.
    worktrees: Vec<(Vec<u8>, Worktree)>,
}

impl Places {
    /// The places of `remotes`, each a remote's name and one URL it is given; of `roots`,
    /// each a working tree's root; and of `git_directories`, each absolute, the common one
    /// first and then each directly under its `worktrees/`. A relative path is read `from`
    /// where `ReadFrom` says, for the URLs here and for every word asked.
    pub fn new<'a>(
        remotes: impl IntoIterator<Item = (&'a [u8], &'a [u8])>,
        roots: impl IntoIterator<Item = &'a [u8]>,
        git_directories: impl IntoIterator<Item = &'a [u8]>,
        from: ReadFrom,
    ) -> Places {
        let mut places = Places {
            root: from.root.to_path_buf(),
            outside: from.outside.map(Path::to_path_buf),
            home: from.home.map(<[u8]>::to_vec),
            urls: BTreeMap::new(),
            paths: BTreeMap::new(),
            worktrees: Vec::new(),
        };
        for (name, url) in remotes {
            let place = Place::Remote {
                name: name.to_vec(),
                url: url.to_vec(),
            };
            for form in destination::forms(url, from) {
                places.urls.entry(form).or_insert_with(|| place.clone());
            }
        }
        let mut path = |path: &[u8], place: Place| {
            places
                .paths
                .entry(destination::canonical(path))
                .or_insert(place);
        };
        for root in roots {
            let entry = [root, b"/.git"].concat();
            path(root, Place::WorkingTree(root.to_vec()));
            path(&entry, Place::GitEntry(entry.clone()));
        }
        for (at, directory) in git_directories.into_iter().enumerate() {
            path(directory, Place::GitDirectory(directory.to_vec()));
            let worktree = match at {
                0 => Worktree::Main,
                _ => Worktree::Linked(last_component(directory).to_vec()),
            };
            places.worktrees.push((directory.to_vec(), worktree));
        }
        places
    }

    /// The worktree whose private repository Git opens for `url`, a URL it sends to: `dupe`
    /// in that worktree's Git directory, other than `own`, the private repository of the
    /// worktree the command runs in. Asked strictly, since a hint names it as where the push
    /// went. `url` is a local path or a `file://` URL, which Git opens no other way, an
    /// absolute path as `Path::is_absolute` tells included, so that a drive's on Windows is
    /// one. Git opens `X/.git`, then `X`, then `X.git/.git`, then `X.git`, the first that
    /// exists (S13): of the paths Git may open for `url` (`destination`), the repository
    /// exists, and every other that exists is one Git tries only after it, `X.git` or
    /// `X.git/.git` of it, so that Git opens it; a `.git` inside it, which Git tries first,
    /// is one of the others. `None` otherwise.
    pub fn private_repository_named_by(&self, url: &[u8], own: &Path) -> Option<&Worktree> {
        let from = ReadFrom {
            root: &self.root,
            outside: self.outside.as_deref(),
            home: self.home.as_deref(),
        };
        let forms = destination::forms(url, from);
        let absolute = crate::bytes::name(url).is_some_and(|name| Path::new(name).is_absolute());
        let local_only = absolute
            || url.starts_with(b"file://")
            || forms.iter().all(|form| matches!(form, Form::Local(_)));
        if !local_only {
            return None;
        }
        let exists =
            |path: &[u8]| crate::bytes::name(path).is_some_and(|name| Path::new(name).exists());
        let existing: Vec<&[u8]> = forms
            .iter()
            .filter_map(|form| match form {
                Form::Local(path) => Some(path.as_slice()),
                Form::Url { .. } => None,
            })
            .filter(|path| exists(path))
            .collect();
        let own = destination::canonical(crate::bytes::of(own.as_os_str()));
        self.worktrees.iter().find_map(|(directory, worktree)| {
            let private = destination::canonical(&[&directory[..], b"/dupe"].concat());
            let after: Vec<Vec<u8>> = [&b".git"[..], b".git/.git"]
                .iter()
                .map(|suffix| destination::canonical(&[&private[..], suffix].concat()))
                .collect();
            let opened = existing.contains(&private.as_slice())
                && existing
                    .iter()
                    .all(|path| *path == private || after.iter().any(|later| later == path));
            (opened && private != own).then_some(worktree)
        })
    }

    /// The public place `word` names, asked of the word whole and, when it holds `=`, of
    /// the part after its first `=`; `None` when neither names one.
    pub fn named_by(&self, word: &[u8]) -> Option<&Place> {
        let after = word
            .iter()
            .position(|&byte| byte == b'=')
            .map(|at| &word[at + 1..]);
        [Some(word), after]
            .into_iter()
            .flatten()
            .find_map(|part| self.named_by_whole(part))
    }

    /// The public place `destination` names, asked of it whole.
    pub fn named_by_whole(&self, destination: &[u8]) -> Option<&Place> {
        let from = ReadFrom {
            root: &self.root,
            outside: self.outside.as_deref(),
            home: self.home.as_deref(),
        };
        destination::forms(destination, from)
            .into_iter()
            .find_map(|form| {
                let path = match &form {
                    Form::Local(path) => self.paths.get(path),
                    Form::Url { .. } => None,
                };
                path.or_else(|| self.urls.get(&form))
            })
    }
}

/// What follows the last separator of `path`, as the platform calls one.
fn last_component(path: &[u8]) -> &[u8] {
    let after = path
        .iter()
        .rposition(|&byte| std::path::is_separator(char::from(byte)))
        .map_or(0, |at| at + 1);
    &path[after..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::{at_the_top, symlink};
    use std::fs;

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn bytes(path: &Path) -> &[u8] {
        crate::bytes::of(path.as_os_str())
    }

    /// `file://` and the path after a host, on a drive after the empty host alone.
    fn file_url(path: &Path) -> Vec<u8> {
        let path = crate::guards::operand::with_slashes(bytes(path));
        if path.starts_with(b"/") {
            [b"file://localhost", &path[..]].concat()
        } else {
            [b"file:///", &path[..]].concat()
        }
    }

    fn remote(url: &[u8]) -> Place {
        Place::Remote {
            name: b"origin".to_vec(),
            url: url.to_vec(),
        }
    }

    /// A project at `<scratch>/w/repo` with a linked worktree beside it, a public local
    /// remote, and a URL remote.
    #[test]
    fn a_word_names_a_place_by_any_form_and_never_by_a_prefix() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-places-{}", std::process::id())));
        let _ = fs::remove_dir_all(&scratch.0);
        // As Git prints a root: its real path, on Windows without the `\\?\` of a canonical
        // one, which a word never spells and Git would read as `host:path` there (S13).
        let real = fs::canonicalize(std::env::temp_dir()).unwrap();
        let printed = bytes(&real).strip_prefix(br"\\?\").unwrap_or(bytes(&real));
        let base =
            Path::new(crate::bytes::name(printed).unwrap()).join(scratch.0.file_name().unwrap());
        let root = base.join("w/repo");
        let linked = base.join("w/linked");
        let git = root.join(".git");
        let linked_git = git.join("worktrees/linked");
        let bare = base.join("srv/p.git");
        for directory in [
            root.join("sub"),
            linked.clone(),
            linked_git.clone(),
            bare.clone(),
        ] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::write(linked.join(".git"), b"gitdir: x\n").unwrap();
        symlink(&root, base.join("to-root")).unwrap();

        let https = b"https://Example.com/team/project.git";
        let remotes = [(&b"origin"[..], &https[..]), (b"origin", bytes(&bare))];
        let roots = [bytes(&root), bytes(&linked)];
        let directories = [bytes(&git), bytes(&linked_git)];
        let from = ReadFrom {
            root: &root,
            outside: None,
            home: None,
        };
        let places = Places::new(remotes, roots, directories, from);

        let named = |word: &[u8]| places.named_by(word).cloned();
        let path = |path: &Path| bytes(path).to_vec();
        assert_eq!(named(b"git@EXAMPLE.com:team/project"), Some(remote(https)));
        assert_eq!(
            named(b"--mirror=ssh://example.com/team/project.git/"),
            Some(remote(https))
        );
        assert_eq!(
            named(b"x=https://example.com/team/project"),
            Some(remote(https))
        );
        // A local remote `/srv/p.git` is named by `/srv/p`, with or without slashes.
        for word in [
            path(&bare),
            path(&base.join("srv/p")),
            path(&base.join("srv/p//")),
        ] {
            assert_eq!(named(&word), Some(remote(bytes(&bare))), "{word:?}");
        }
        // The root without its drive, and with its drive as a directory, each the root
        // itself where a root alone is absolute.
        let written = crate::guards::operand::with_slashes(bytes(&root));
        let (rooted, drive_as_directory, separated, through_the_shell) = match written.split_at(2) {
            ([letter, b':'], rest) => (
                rest.to_vec(),
                [b"/", &[*letter][..], rest].concat(),
                [b"/", &[*letter][..], b"\\", &rest[1..]].concat(),
                [b"/", &written[..]].concat(),
            ),
            _ => (
                written.clone(),
                written.clone(),
                written.clone(),
                written.clone(),
            ),
        };
        let root_place = Some(Place::WorkingTree(path(&root)));
        for word in [
            path(&root),
            path(&base.join("to-root")),
            b".".to_vec(),
            b"".to_vec(),
            b"x=".to_vec(),
            file_url(&root),
            path(&root.join("absent/..")),
            [b"file://", bytes(&root)].concat(),
            rooted.clone(),
            drive_as_directory.clone(),
            separated.clone(),
            through_the_shell.clone(),
        ] {
            assert_eq!(named(&word), root_place, "{}", word.escape_ascii());
        }
        assert_eq!(
            named(&path(&linked)),
            Some(Place::WorkingTree(path(&linked)))
        );
        assert_eq!(
            named(&path(&linked.join(".git"))),
            Some(Place::GitEntry([bytes(&linked), b"/.git"].concat()))
        );
        assert_eq!(
            named(&path(&linked_git)),
            Some(Place::GitDirectory(path(&linked_git)))
        );

        // Neighbors that name nothing: another case of the path, another host, a path
        // that begins like a place's, `..` from the root, a refspec, an option.
        for word in [
            &b"https://example.com/team/Project.git"[..],
            b"https://example.org/team/project.git",
            b"https://example.com/team/project-private.git",
            b"git@example.com:team/pro%6Aect.git",
            &[bytes(&root), b"-other"].concat(),
            &[bytes(&base), b"/srv/p-private.git"].concat(),
            b"..",
            b"main:leak",
            b"-v",
            b"add",
        ] {
            assert_eq!(named(word), None, "{}", word.escape_ascii());
        }
    }

    /// The private repository of another worktree is `dupe` in its Git directory, whatever
    /// spelling of its path a URL holds, never the one the command runs in, and only where
    /// it is the one path Git may open for the URL that exists.
    #[test]
    fn a_url_names_another_worktree_s_private_repository_where_git_opens_only_it() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-private-{}", std::process::id())));
        let _ = fs::remove_dir_all(&scratch.0);
        let real = fs::canonicalize(std::env::temp_dir()).unwrap();
        let printed = bytes(&real).strip_prefix(br"\\?\").unwrap_or(bytes(&real));
        let base =
            Path::new(crate::bytes::name(printed).unwrap()).join(scratch.0.file_name().unwrap());
        let root = base.join("repo");
        let common = root.join(".git");
        let linked = common.join("worktrees/agent");
        let (main_private, linked_private) = (common.join("dupe"), linked.join("dupe"));
        for directory in [&main_private, &linked_private, &base.join("both")] {
            fs::create_dir_all(directory).unwrap();
        }
        // `both` exists, and `both.git` leads to the main worktree's private repository:
        // Git opens `both`, so the URL names neither.
        symlink(&main_private, base.join("both.git")).unwrap();
        let from = ReadFrom {
            root: &root,
            outside: None,
            home: None,
        };
        let places = Places::new([], [], [bytes(&common), bytes(&linked)], from);
        let agent = Worktree::Linked(b"agent".to_vec());
        let path = |path: &Path| bytes(path).to_vec();
        for (url, own, named) in [
            (
                b".git/dupe".to_vec(),
                &linked_private,
                Some(&Worktree::Main),
            ),
            (
                b".git/dupe/".to_vec(),
                &linked_private,
                Some(&Worktree::Main),
            ),
            (
                b".git/sub/../dupe".to_vec(),
                &linked_private,
                Some(&Worktree::Main),
            ),
            (path(&main_private), &linked_private, Some(&Worktree::Main)),
            (
                file_url(&main_private),
                &linked_private,
                Some(&Worktree::Main),
            ),
            (
                b".git/worktrees/agent/dupe".to_vec(),
                &main_private,
                Some(&agent),
            ),
            (b".git/dupe".to_vec(), &main_private, None),
            (b".git/dupe-other".to_vec(), &linked_private, None),
            (b".git".to_vec(), &linked_private, None),
            (path(&base.join("both")), &linked_private, None),
            (
                b"https://example.com/repo/.git/dupe".to_vec(),
                &linked_private,
                None,
            ),
        ] {
            assert_eq!(
                places.private_repository_named_by(&url, own),
                named,
                "{}",
                url.escape_ascii()
            );
        }
        // A sibling `dupe.git` is a path Git tries only after `dupe` itself.
        fs::create_dir(common.join("dupe.git")).unwrap();
        assert_eq!(
            places.private_repository_named_by(b".git/dupe", &linked_private),
            Some(&Worktree::Main)
        );
        // A `.git` inside it is what Git opens first for that path, so it names none.
        fs::create_dir(linked_private.join(".git")).unwrap();
        assert_eq!(
            places.private_repository_named_by(b".git/worktrees/agent/dupe", &main_private),
            None
        );
        assert_eq!(Worktree::Main.named(), b"the main worktree");
        assert_eq!(agent.named(), b"worktree agent");
    }

    #[test]
    fn a_relative_word_and_a_relative_url_are_read_from_the_same_places() {
        let root = at_the_top("/nonexistent-git-dupe/w/repo");
        let outside = at_the_top("/nonexistent-git-dupe/w");
        let name = |path| Path::new(crate::bytes::name(path).unwrap());
        let from = ReadFrom {
            root: name(&root),
            outside: Some(name(&outside)),
            home: None,
        };
        let places = Places::new([(&b"up"[..], &b"../up.git"[..])], [], [], from);
        let up = Place::Remote {
            name: b"up".to_vec(),
            url: b"../up.git".to_vec(),
        };
        assert_eq!(places.named_by(b"../up"), Some(&up));
        let absolute = at_the_top("/nonexistent-git-dupe/w/up");
        assert_eq!(places.named_by(&absolute), Some(&up));
        // From the user's directory outside the root, Git opens `up` at `<outside>/up.git`,
        // which the URL names when read from the root.
        assert_eq!(places.named_by(b"up"), Some(&up));
        assert_eq!(places.named_by(b"down"), None);
    }

    #[test]
    fn a_word_and_a_url_beginning_with_a_tilde_are_read_from_the_same_home() {
        let home = at_the_top("/nonexistent-git-dupe/h");
        let from = ReadFrom {
            root: Path::new("/nonexistent-git-dupe/w/repo"),
            outside: None,
            home: Some(&home),
        };
        let places = Places::new([(&b"up"[..], &b"~/up.git"[..])], [], [], from);
        let up = Place::Remote {
            name: b"up".to_vec(),
            url: b"~/up.git".to_vec(),
        };
        let absolute = at_the_top("/nonexistent-git-dupe/h/up");
        assert_eq!(places.named_by(&absolute), Some(&up));
        assert_eq!(places.named_by(b"--repo=~/up/"), Some(&up));
        assert_eq!(places.named_by(b"~up"), None);
    }
}
