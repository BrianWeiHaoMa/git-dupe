//! What `git dupe clean` deletes, observed beside plain `git clean`: a workspace built
//! once, attached and settled, is copied whole before each command, `git dupe clean WORDS`
//! runs in one copy and plain `git clean WORDS` in the other, with the same words, from
//! the same directory below the root, with the same standard input, the same global
//! options before the command, and the same variables added to the environment, under the
//! same release. Plain Git under each release says
//! what it deletes, so no scenario writes a per-release expectation or reads a line of
//! Git's.
//!
//! Over every path that stood in the workspace before the command, the public `.git`
//! apart, the comparison observes:
//!
//! 1. At or below a spared path: it stands in the command's copy as before, byte for
//!    byte, and a spared path with nothing at it still has nothing.
//! 2. Neither at, below, nor above a spared path, and gone from plain Git's copy: gone
//!    from the command's, except, for words with `-X`, the kept paths, which must be
//!    exactly the ones that stand (G16: of a directory that holds a hidden path and that
//!    plain `git clean -X` takes as ignored as a whole, the files outside the user's
//!    directory and pathspecs when either lies inside it, but for the outermost
//!    directory in it that holds no hidden path and holds either, which Git takes whole).
//! 3. Such a path still in plain Git's copy: still in the command's, except, for words
//!    with `-X`, the admitted paths, which must be exactly the ones gone (G16's
//!    exception: the ignored files of such a directory that Git's own walk reaches once
//!    it is opened, at or below the user's directory and under the user's pathspecs and
//!    in no entry Git takes whole: a nested repository, an ignored directory that holds
//!    no hidden path, and, without `-d` or a pathspec, a directory holding only these,
//!    directories at hidden paths, and ignored files).
//!
//! A directory above a spared path is not compared: plain Git may remove it whole where
//! the command keeps it for what it holds. The exit statuses are compared, except where
//! plain Git, given `-C` naming a directory it removes whole, empties it and then fails
//! on `./`, while the command keeps the directory for the spared path in it: the
//! scenario names that case, and the command then exits 0 where plain Git does not. Run
//! from inside such a directory instead, Git refuses to remove the directory it runs in,
//! and both exit 0.
//!
//! The spared set of a fixture is every hidden path, with the outermost nested
//! repository above a hidden path standing in place of the hidden paths below it: a
//! directory that holds a `.git` entry and no path the index Git's `clean` reads tracks,
//! the public index unless the comparison names another. No fixture holds a hidden path
//! beyond a symbolic link: `clean` refuses every form while one does (G16), and
//! `clean_cases` checks that refusal on its own.

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::attached::{private_add, private_commit};
use super::files::{copy, write};
use super::output::{End, Output};
use super::scenario::Scenario;
use super::tree::Tree;

/// A workspace built once, from which each comparison copies the command's workspace and
/// plain Git's.
pub struct Twin<'s> {
    s: &'s Scenario,
    built: PathBuf,
    spared: Vec<String>,
    copies: Cell<usize>,
}

impl<'s> Twin<'s> {
    /// The workspace at `built`, attached and settled, whose spared set is `spared`.
    pub fn of(s: &'s Scenario, built: &Path, spared: &[&str]) -> Twin<'s> {
        Twin {
            s,
            built: built.to_path_buf(),
            spared: spared.iter().map(|path| (*path).to_owned()).collect(),
            copies: Cell::new(0),
        }
    }

    /// A comparison of `git dupe clean WORDS` with plain `git clean WORDS`, from the root,
    /// with no input, nothing admitted or kept, and the statuses compared.
    pub fn clean<'t>(&'t self, words: &[&'t str]) -> Comparison<'t, 's> {
        Comparison {
            twin: self,
            words: words.to_vec(),
            directory: Directory::Root,
            input: None,
            options: Vec::new(),
            variables: Vec::new(),
            admitted: Vec::new(),
            kept: Vec::new(),
            plain_fails: false,
            same_output: false,
        }
    }
}

/// One comparison, not yet run.
pub struct Comparison<'t, 's> {
    twin: &'t Twin<'s>,
    words: Vec<&'t str>,
    directory: Directory<'t>,
    input: Option<&'t [u8]>,
    /// Git's global options before the command, in order.
    options: Vec<&'t str>,
    variables: Vec<(&'t str, &'t str)>,
    admitted: Vec<&'t str>,
    kept: Vec<&'t str>,
    plain_fails: bool,
    same_output: bool,
}

/// Where a comparison runs both.
#[derive(Clone, Copy, Debug)]
enum Directory<'t> {
    Root,
    /// From this directory below the root.
    Below(&'t str),
    /// From the root, with `-C` naming this directory below it before the command.
    NamedByC(&'t str),
}

impl<'t> Comparison<'t, '_> {
    /// Runs both from `directory`, a path below the root.
    pub fn below(mut self, directory: &'t str) -> Self {
        self.directory = Directory::Below(directory);
        self
    }

    /// Runs both from the root with `-C <directory>` before the command, `directory`
    /// being a path below the root.
    pub fn named_by_c(mut self, directory: &'t str) -> Self {
        self.directory = Directory::NamedByC(directory);
        self
    }

    /// Feeds both `bytes` on standard input.
    pub fn input(mut self, bytes: &'t [u8]) -> Self {
        self.input = Some(bytes);
        self
    }

    /// Gives both `-c <setting>` before the command.
    pub fn setting(mut self, setting: &'t str) -> Self {
        self.options.extend(["-c", setting]);
        self
    }

    /// Gives both Git's global option `option`, a word of its own, before the command.
    pub fn global_option(mut self, option: &'t str) -> Self {
        self.options.push(option);
        self
    }

    /// Gives both the environment variable `name` set to `value`, as a caller's
    /// environment holds it; a relative path names a path in each copy.
    pub fn variable(mut self, name: &'t str, value: &'t str) -> Self {
        self.variables.push((name, value));
        self
    }

    /// The command's standard output must be plain Git's, byte for byte: for words under
    /// which the two delete or list the same paths.
    pub fn same_output(mut self) -> Self {
        self.same_output = true;
        self
    }

    /// The paths G16 admits for these words: exactly these are gone from the command's
    /// copy among the paths plain Git kept.
    pub fn admitting(mut self, paths: &[&'t str]) -> Self {
        self.admitted = paths.to_vec();
        self
    }

    /// The paths G16 keeps for these words: exactly these stand in the command's copy
    /// among the paths gone from plain Git's.
    pub fn keeping(mut self, paths: &[&'t str]) -> Self {
        self.kept = paths.to_vec();
        self
    }

    /// The case named above: given `-C` naming a directory plain Git removes whole, plain
    /// Git fails on `./`, and the command, which keeps it, exits 0.
    pub fn plain_fails(mut self) -> Self {
        self.plain_fails = true;
        self
    }

    /// Runs both, makes the three observations and compares the statuses, and returns what
    /// the command printed.
    pub fn run(self) -> Output {
        let twin = self.twin;
        let number = twin.copies.get();
        twin.copies.set(number + 1);
        let name = twin.built.file_name().unwrap().to_str().unwrap();
        let parent = twin.built.parent().unwrap();
        let ours = parent.join(format!("{name}-{number}-dupe"));
        let plains = parent.join(format!("{name}-{number}-plain"));
        copy(&twin.built, &ours);
        copy(&twin.built, &plains);
        let before = Tree::of(&ours);

        let (below, named) = match self.directory {
            Directory::Root => ("", None),
            Directory::Below(below) => (below, None),
            Directory::NamedByC(named) => ("", Some(named)),
        };
        let before_command: Vec<&str> = named
            .map(|named| ["-C", named])
            .into_iter()
            .flatten()
            .chain(self.options.iter().copied())
            .collect();
        let words = |command: &'t [&'t str]| {
            before_command
                .iter()
                .chain(command)
                .chain(&self.words)
                .copied()
                .collect::<Vec<_>>()
        };
        let mut command = twin
            .s
            .git(words(&["dupe", "clean"]))
            .from(&ours.join(below));
        let mut plain = twin.s.git(words(&["clean"])).from(&plains.join(below));
        if let Some(bytes) = self.input {
            command = command.input(bytes);
            plain = plain.input(bytes);
        }
        for (name, value) in &self.variables {
            command = command.variable(name, value);
            plain = plain.variable(name, value);
        }
        let output = command.run();
        let plain = plain.run();
        let after = Tree::of(&ours);
        let context = format!(
            "clean {:?} from {:?}: git dupe {output:?}; plain git {plain:?}",
            self.words, self.directory
        );

        // 1. Every spared path, and everything below it, as before.
        for spared in &twin.spared {
            assert!(
                before.same_at_or_below(&after, &ours.join(spared)),
                "{spared:?} changed; {context}"
            );
        }
        // 2. and 3. Every other path as plain Git left it, but the kept and the admitted.
        let public = ours.join(".git");
        let mut kept_beyond_plain = Vec::new();
        let mut gone_beyond_plain = Vec::new();
        for path in before.paths().filter(|path| !path.starts_with(&public)) {
            let relative = path.strip_prefix(&ours).unwrap();
            let directory =
                fs::symlink_metadata(twin.built.join(relative)).is_ok_and(|found| found.is_dir());
            let related = twin.spared.iter().any(|spared| {
                let spared = Path::new(spared);
                relative.starts_with(spared) || (directory && spared.starts_with(relative))
            });
            if related {
                continue;
            }
            let gone = !after.holds(path);
            let gone_from_plain = fs::symlink_metadata(plains.join(relative)).is_err();
            if gone_from_plain && !gone {
                kept_beyond_plain.push(relative.to_str().unwrap().to_owned());
            } else if gone && !gone_from_plain {
                gone_beyond_plain.push(relative.to_str().unwrap().to_owned());
            }
        }
        kept_beyond_plain.sort();
        gone_beyond_plain.sort();
        assert_eq!(
            kept_beyond_plain,
            sorted(&self.kept),
            "kept beyond plain Git; {context}"
        );
        assert_eq!(
            gone_beyond_plain,
            sorted(&self.admitted),
            "gone beyond plain Git; {context}"
        );

        if self.same_output {
            assert_eq!(output.stdout, plain.stdout, "standard output; {context}");
        }
        if self.plain_fails {
            assert_eq!(output.end, End::Code(0), "{context}");
            assert_ne!(plain.end, End::Code(0), "{context}");
        } else {
            assert_eq!(output.end, plain.end, "{context}");
        }
        output
    }
}

fn sorted(paths: &[&str]) -> Vec<String> {
    let mut paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
    paths.sort();
    paths
}

/// The spared set of `hidden_path_of_every_kind`.
pub const EVERY_KIND_SPARED: [&str; 16] = [
    ".env.local",
    ".gitdupe",
    "absent",
    "allign/keep.txt",
    "build/local.cfg",
    "cr\rx",
    "dangling",
    "docs/notes.md",
    "fake",
    "ig *[d/keep !#.txt",
    "ign/deep/keep.txt",
    "notes",
    "swapped-dir",
    "swapped-file",
    "vendor/nested",
    "w *[#!x",
];

/// Makes `dir` an attached and settled workspace holding a hidden path of every kind
/// beside what `git clean` deletes, keeps, or enters under each of its forms. The project
/// ignores `.env.local`, `.vscode/`, `build/`, `ign/`, `*.o`, and `/ig *[d/`, and tracks
/// `.gitignore`, `README.md`, `docs/design.md`, and `src/main.c`. Hidden, with their
/// neighbors:
///
/// - files: `.env.local`, `docs/notes.md`, `.gitdupe` (privately tracked), and
///   `w *[#!x` and `cr<CR>x`, names Git reads as patterns or that a rule must bracket;
/// - a directory, `notes`, with a privately tracked file, untracked ones, and an ignored
///   one;
/// - `absent`, with nothing at it;
/// - `swapped-file`, privately tracked as a file, a directory on disk, and `swapped-dir`,
///   listed, a file on disk;
/// - inside the ignored `build/`, `build/local.cfg` beside `build/out.js` and
///   `build/sub/x.o`; two levels into the ignored `ign/`, `ign/deep/keep.txt` beside
///   `ign/junk.txt`, `ign/deep/junk2.txt`, and `ign/other/z.txt`; inside the ignored
///   `ig *[d/`, `ig *[d/keep !#.txt` beside `ig *[d/junk.txt`;
/// - inside `allign/`, which no rule names, `allign/keep.txt` beside its one other entry,
///   the ignored `allign/a.o`;
/// - inside untracked nested repositories, each with another file: `vendor/nested`, one
///   Git opens; `fake`, whose `.git` is an empty file; `dangling`, whose `.git` is a
///   symbolic link to nothing.
///
/// Not hidden: `scratch.txt`, `main.o`, `src/gen.tmp`, `src/gen.o`, `untracked-dir/`,
/// `.vscode/launch.json`, `vendor/loose.txt`, the nested repository `solo-nested`, and
/// the empty `empty-dir`. The spared set is `EVERY_KIND_SPARED`.
pub fn hidden_path_of_every_kind(s: &Scenario, dir: &Path) {
    s.attached_project(dir);
    write(
        dir,
        ".gitignore",
        b".env.local\n.vscode/\nbuild/\nign/\n*.o\n/ig\\ \\*\\[d/\n",
    );
    write(dir, "src/main.c", b"int main;\n");
    s.git(["add", "--", ".gitignore", "src/main.c"])
        .from(dir)
        .succeeds();
    s.commit_public(dir);

    for path in [
        ".env.local",
        "allign/keep.txt",
        "allign/a.o",
        "build/local.cfg",
        "build/out.js",
        "build/sub/x.o",
        "cr\rx",
        "dangling/secret.txt",
        "dangling/other.txt",
        "docs/notes.md",
        "fake/in/secret.txt",
        "fake/junk.txt",
        "ig *[d/keep !#.txt",
        "ig *[d/junk.txt",
        "ign/deep/keep.txt",
        "ign/junk.txt",
        "ign/deep/junk2.txt",
        "ign/other/z.txt",
        "notes/today.md",
        "notes/scratch.md",
        "notes/sub/deep.md",
        "notes/a.o",
        "swapped-file",
        "swapped-dir",
        "vendor/nested/secret.txt",
        "vendor/nested/code.c",
        "vendor/loose.txt",
        "w *[#!x",
        "scratch.txt",
        "main.o",
        "src/gen.tmp",
        "src/gen.o",
        "untracked-dir/f.txt",
        "untracked-dir/g.o",
        ".vscode/launch.json",
        "solo-nested/f.txt",
    ] {
        write(dir, path, format!("{path}\n").as_bytes());
    }
    fs::create_dir(dir.join("empty-dir")).unwrap();
    fs::write(dir.join("fake/.git"), b"").unwrap();
    symlink(dir.join("nowhere"), dir.join("dangling/.git")).unwrap();
    for nested in ["vendor/nested", "solo-nested"] {
        s.git(["init", "-q"]).from(&dir.join(nested)).succeeds();
    }
    write(
        dir,
        ".gitdupe",
        b"absent\nallign/keep.txt\ncr\rx\ndangling/secret.txt\nfake/in/secret.txt\n\
          ig *[d/keep !#.txt\nign/deep/keep.txt\nnotes\nswapped-dir\nvendor/nested/secret.txt\n\
          w *[#!x\n",
    );
    for path in [
        ".gitdupe",
        ".env.local",
        "build/local.cfg",
        "docs/notes.md",
        "notes/today.md",
        "swapped-file",
    ] {
        private_add(s, dir, path);
    }
    private_commit(s, dir);
    fs::remove_file(dir.join("swapped-file")).unwrap();
    write(dir, "swapped-file/inner.txt", b"inner\n");

    // Settled: the region holds the hidden paths, and nothing is exposed.
    let settled = s.git(["dupe", "status", "--short"]).from(dir).run();
    assert_eq!(settled.end, End::Code(0), "{settled:?}");
    assert!(settled.lines("warning").is_empty(), "{settled:?}");
}
