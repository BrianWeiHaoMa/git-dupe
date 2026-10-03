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

/// The public places of one command, and where a relative path is read from.
pub struct Places {
    root: PathBuf,
    outside: Option<PathBuf>,
    home: Option<Vec<u8>>,
    /// Every form of every public URL, with the first URL that has it.
    urls: BTreeMap<Form, Place>,
    /// The canonical path of every root, `.git` entry, and Git directory.
    paths: BTreeMap<Vec<u8>, Place>,
}

impl Places {
    /// The places of `remotes`, each a remote's name and one URL it is given; of `roots`,
    /// each a working tree's root; and of `git_directories`, each absolute. A relative path
    /// is read `from` where `ReadFrom` says, for the URLs here and for every word asked.
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
        for directory in git_directories {
            path(directory, Place::GitDirectory(directory.to_vec()));
        }
        places
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn bytes(path: &Path) -> &[u8] {
        path.as_os_str().as_bytes()
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
        let base = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(scratch.0.file_name().unwrap());
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
        let path = |path: &Path| path.as_os_str().as_bytes().to_vec();
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
        let root_place = Some(Place::WorkingTree(path(&root)));
        for word in [
            path(&root),
            path(&base.join("to-root")),
            b".".to_vec(),
            b"".to_vec(),
            b"x=".to_vec(),
            [b"file://localhost", bytes(&root)].concat(),
            path(&root.join("absent/..")),
        ] {
            assert_eq!(named(&word), root_place, "{}", word.escape_ascii());
        }
        assert_eq!(
            named(&path(&linked)),
            Some(Place::WorkingTree(path(&linked)))
        );
        assert_eq!(
            named(&path(&linked.join(".git"))),
            Some(Place::GitEntry(path(&linked.join(".git"))))
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

    #[test]
    fn a_relative_word_and_a_relative_url_are_read_from_the_same_places() {
        let root = Path::new("/nonexistent-git-dupe/w/repo");
        let outside = Path::new("/nonexistent-git-dupe/w");
        let from = ReadFrom {
            root,
            outside: Some(outside),
            home: None,
        };
        let places = Places::new([(&b"up"[..], &b"../up.git"[..])], [], [], from);
        let up = Place::Remote {
            name: b"up".to_vec(),
            url: b"../up.git".to_vec(),
        };
        assert_eq!(places.named_by(b"../up"), Some(&up));
        assert_eq!(places.named_by(b"/nonexistent-git-dupe/w/up"), Some(&up));
        // From the user's directory outside the root, Git opens `up` at `<outside>/up.git`,
        // which the URL names when read from the root.
        assert_eq!(places.named_by(b"up"), Some(&up));
        assert_eq!(places.named_by(b"down"), None);
    }

    #[test]
    fn a_word_and_a_url_beginning_with_a_tilde_are_read_from_the_same_home() {
        let from = ReadFrom {
            root: Path::new("/nonexistent-git-dupe/w/repo"),
            outside: None,
            home: Some(b"/nonexistent-git-dupe/h"),
        };
        let places = Places::new([(&b"up"[..], &b"~/up.git"[..])], [], [], from);
        let up = Place::Remote {
            name: b"up".to_vec(),
            url: b"~/up.git".to_vec(),
        };
        assert_eq!(places.named_by(b"/nonexistent-git-dupe/h/up"), Some(&up));
        assert_eq!(places.named_by(b"--repo=~/up/"), Some(&up));
        assert_eq!(places.named_by(b"~up"), None);
    }
}
