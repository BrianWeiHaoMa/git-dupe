//! `git dupe clean` given a pathspec that is not a literal path while a hidden path lies
//! below a loose directory, one that is not hidden and under which the project tracks
//! nothing: refused, deleting nothing, in every spelling and every option form, because
//! Git's `clean`, given a pathspec, deletes such a directory whole unless its walk met
//! inside it an entry a rule excludes, and such a pathspec can keep the hidden path from
//! being met (G16, `Holds/G16`, S4). Literal pathspecs, and every pathspec where no loose
//! directory is, run as plain `git clean` runs, observed beside it.

use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Runs, Scenario, Tree, Twin, changed_since, holds, names, private_add,
    private_commit, refusal, region_rules, run_traced, under_each_release, write,
};

/// An attached project whose committed `.gitignore` ignores `ign/` and `*.o`, beside its
/// committed `README.md` and `docs/design.md`.
fn project(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    s.attached_project(&dir);
    write(&dir, ".gitignore", b"ign/\n*.o\n");
    s.git(["add", "--", ".gitignore"]).from(&dir).succeeds();
    s.commit_public(&dir);
    dir
}

/// `project` with hidden paths below loose directories: `cache/secret` beside
/// `cache/junk` and the ignored `cache/build.o`, `deep/a/secret3` beside `deep/a/junk3`
/// and `deep/junk4`, and `ign/secret4`, under the ignored `ign/`, beside `ign/junk5`,
/// each privately tracked; settled.
fn loose_workspace(s: &Scenario) -> PathBuf {
    let dir = project(s, "loose");
    for path in [
        "cache/secret",
        "cache/junk",
        "cache/build.o",
        "deep/a/secret3",
        "deep/a/junk3",
        "deep/junk4",
        "ign/secret4",
        "ign/junk5",
    ] {
        write(&dir, path, format!("{path}\n").as_bytes());
    }
    for path in ["cache/secret", "deep/a/secret3", "ign/secret4"] {
        private_add(s, &dir, path);
    }
    private_commit(s, &dir);
    settled(s, &dir);
    dir
}

const LOOSE_SPARED: [&str; 4] = [".gitdupe", "cache/secret", "deep/a/secret3", "ign/secret4"];

fn settled(s: &Scenario, dir: &Path) {
    let settled = s.git(["dupe", "status"]).from(dir).run();
    assert_eq!(settled.end, End::Code(0), "{settled:?}");
    assert!(settled.lines("warning").is_empty(), "{settled:?}");
}

/// `output` is the refusal of `word`, as typed, naming `directory` as the outermost loose
/// directory above `path`, and, unless `only_ignored`, `-e` as the way to keep a path;
/// nothing in the working tree at `dir` changed since `before`.
fn refused(
    output: &Output,
    word: &[u8],
    directory: &[u8],
    path: &[u8],
    only_ignored: bool,
    before: &Tree,
    dir: &Path,
) {
    let typed = if word.contains(&b'\n') {
        // Between double quotes, as Git quotes a path that needs it.
        let escaped: Vec<u8> = word
            .iter()
            .flat_map(|&byte| match byte {
                b'\n' => b"\\n".to_vec(),
                other => vec![other],
            })
            .collect();
        [&b"\""[..], &escaped, b"\""].concat()
    } else {
        [&b"'"[..], word, b"'"].concat()
    };
    let line = refusal(output, &[&typed, path, b"which is not a literal path"]);
    names(
        line,
        &[b"delete ", directory, b", which is not hidden"].concat(),
    );
    names(line, b"give literal pathspecs instead");
    assert_eq!(holds(line, b"-e <pattern>"), !only_ignored, "{output:?}");
    let changed = changed_since(before, dir);
    assert!(changed.is_empty(), "changed {changed:?}: {output:?}");
}

/// The words of each run git-dupe itself started whose command word is `command`.
fn words_of(runs: &Runs, command: &str) -> Vec<Vec<Vec<u8>>> {
    let own = runs.own();
    own.words()
        .iter()
        .zip(own.commands())
        .filter(|(_, word)| *word == command.as_bytes())
        .map(|(words, _)| words.clone())
        .collect()
}

fn listed_under(path: &str) -> Vec<u8> {
    format!(":(top,literal){path}").into_bytes()
}

#[test]
fn a_non_literal_pathspec_over_a_loose_directory_is_refused_in_every_spelling_and_form() {
    under_each_release(|s| {
        let dir = loose_workspace(s);
        let before = Tree::working(&dir);
        // Each names `cache` and `cache/secret`, the first region path below one.
        let names_cache = |output: &Output, word: &str, only_ignored: bool| {
            refused(
                output,
                word.as_bytes(),
                b"cache",
                b"cache/secret",
                only_ignored,
                &before,
                &dir,
            );
        };
        let clean = |words: &[&str]| {
            s.git(["dupe", "clean"].iter().chain(words))
                .from(&dir)
                .run()
        };
        for words in [
            &["-fdx", "--", "cache", ":(exclude)cache/secret"][..],
            &["-fdx", "--", ":(exclude)cache/secret"],
            &["-fd", "--", ":!cache/secret"],
            &["-fd", "--", ":^cache/secret"],
            // No `--`: a word that is no option is a pathspec too.
            &["-fd", ":(exclude,glob)**/secret"],
            &["-fd", "--", ":(attr:-text)cache"],
            // An exclusion naming no hidden path, and one naming nothing.
            &["-fd", "--", ":(exclude)cache/junk"],
            &["-n", "--", ":(exclude)nothing-here"],
            &["-f", "--", "cache", ":(exclude)cache/secret"],
            &["-fx", "--", ":(exclude)deep/a/secret3"],
            &["-i", "--", ":(exclude)cache/secret"],
            // Patterns that match the directory and not the file below it, and safe
            // ones the bytes alone cannot tell from them.
            &["-fd", "--", "cache?"],
            &["-f", "--", "cache*[k/]"],
            &["-fd", "--", "*[k/]"],
            &["-fd", "--", "cache*[!t]"],
            &["-fd", "--", ":(glob)cache*[k/]"],
            &["-fd", "--", ":(icase)CACHE*[K/]"],
            &["-fd", "--", "cache\\/"],
            &["-fd", "--", "*e/"],
            &["-fd", "--", "cache*"],
            &["-fd", "--", ":(top)cache"],
            &["-fd", "--", ":(literal)cache"],
            &["-fd", "--", ":/cache*"],
            // Git reads short magic after `:/`: an exclusion there, and `top` alone, as
            // `:(top)cache` is; only `:/` itself is a literal path.
            &["-fd", "--", ":/!cache/secret"],
            &["-fd", "--", ":/^cache/secret"],
            &["-fd", "--", ":/cache"],
            // An `-e` lacking its value, which Git refuses: refused here first.
            &["-fd", "cache?", "-e"],
        ] {
            let word = words
                .iter()
                .skip(1)
                .find(|word| **word != "--" && !word.starts_with('-') && **word != "cache")
                .unwrap();
            names_cache(&clean(words), word, false);
        }
        // Under `-X`, which refuses `-e`, the way out is literal pathspecs alone.
        for words in [
            &["-fdX", "--", "ign", ":(exclude)ign/secret4"][..],
            &["-fX", "--", ":(exclude)ign/secret4"],
        ] {
            names_cache(&clean(words), ":(exclude)ign/secret4", true);
        }
        // From inside the loose directory, and under a setting that makes Git read the
        // word as a literal path: the bytes decide.
        let inside = s
            .git(["dupe", "clean", "-f", "--", ".", ":(exclude)secret"])
            .from(&dir.join("cache"))
            .run();
        names_cache(&inside, ":(exclude)secret", false);
        for word in [":(exclude)cache/secret", "cache?"] {
            let words = ["--literal-pathspecs", "dupe", "clean", "-fd", "--", word];
            names_cache(&s.git(words).from(&dir).run(), word, false);
        }
        // A word holding a newline is named on the one line, quoted as Git quotes it.
        names_cache(
            &clean(&["-fd", "--", ":(exclude)a\nb"]),
            ":(exclude)a\nb",
            false,
        );
    });
}

/// The refusal comes before the `clean` run, after the one public listing, which is
/// asked under the outermost ancestor of every region path, each at once; it settles,
/// as every outcome of `clean` does; and it names the first region path in byte order
/// that lies below a loose directory (G16, `Holds/G16`, G22, R3).
#[test]
fn the_refusal_follows_one_listing_under_every_outermost_ancestor_and_settles() {
    under_each_release(|s| {
        let dir = loose_workspace(s);
        let log = s.dir().join("trace");
        // A `.gitdupe` written by hand since the last command, listing one more path.
        write(&dir, ".gitdupe", b"newdir/x\n");
        let before = Tree::working(&dir);
        let (output, runs) = run_traced(
            s.git([
                "dupe",
                "clean",
                "-fdx",
                "--",
                "cache",
                ":(exclude)cache/secret",
            ])
            .from(&dir),
            &log,
        );
        refused(
            &output,
            b":(exclude)cache/secret",
            b"cache",
            b"cache/secret",
            false,
            &before,
            &dir,
        );
        assert!(words_of(&runs, "clean").is_empty(), "{runs:?}");
        let listings: Vec<_> = words_of(&runs, "ls-files")
            .into_iter()
            .filter(|words| words.contains(&listed_under("cache")))
            .collect();
        assert_eq!(listings.len(), 1, "{runs:?}");
        for ancestor in ["deep", "ign", "newdir"] {
            assert!(listings[0].contains(&listed_under(ancestor)), "{runs:?}");
        }
        assert!(
            region_rules(&dir).contains(&b"/newdir/x".to_vec()),
            "{output:?}"
        );

        // A literal pathspec: no listing under an ancestor, and the run is made.
        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-n", "--", "cache"]).from(&dir),
            &log,
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(words_of(&runs, "clean").len(), 1, "{runs:?}");
        assert!(
            !words_of(&runs, "ls-files")
                .iter()
                .any(|words| words.contains(&listed_under("cache"))),
            "{runs:?}"
        );

        // Once the project tracks a path under `cache`, the next loose directory is named.
        write(&dir, "cache/README", b"public\n");
        s.git(["add", "--", "cache/README"]).from(&dir).succeeds();
        s.commit_public(&dir);
        let before = Tree::working(&dir);
        let output = s
            .git(["dupe", "clean", "-fd", "--", ":(exclude)nothing-here"])
            .from(&dir)
            .run();
        refused(
            &output,
            b":(exclude)nothing-here",
            b"deep",
            b"deep/a/secret3",
            false,
            &before,
            &dir,
        );
    });
}

#[test]
fn literal_pathspecs_over_a_loose_directory_run_as_plain_git_clean_runs() {
    under_each_release(|s| {
        let dir = loose_workspace(s);
        let twin = Twin::of(s, &dir, &LOOSE_SPARED);
        for words in [
            &["-fd", "--", "cache"][..],
            &["-fd", "--", "cache/"],
            &["-f", "--", "cache", "deep/a"],
            &["-fdx"],
            &["-fd", "--", ":/"],
            &["-fdX", "--", "ign"],
            // A pattern of `-e` is no pathspec.
            &["-fd", "-e", "*.o", "--", "cache"],
        ] {
            twin.clean(words).run();
        }
        twin.clean(&["-fd", "--", "../cache"]).below("docs").run();
    });
}

/// Where every ancestor of a region path holds a path the project tracks, or a region
/// path has no ancestor, no directory is loose, and every pathspec runs: a hidden
/// directory at the root is nobody's loose directory, the privately tracked file below it
/// included. Then a hidden file one level below a tracked directory, in a directory the
/// project tracks nothing under, has that deeper directory loose.
#[test]
fn without_a_loose_directory_a_non_literal_pathspec_runs_and_a_deeper_one_is_found() {
    under_each_release(|s| {
        let dir = project(s, "tracked");
        for path in ["docs/notes.md", "docs/junk", "secret", "notes/today.md"] {
            write(&dir, path, format!("{path}\n").as_bytes());
        }
        write(&dir, "scratch.txt", b"scratch\n");
        write(&dir, ".gitdupe", b"notes\nsecret\n");
        for path in [".gitdupe", "docs/notes.md", "notes/today.md"] {
            private_add(s, &dir, path);
        }
        private_commit(s, &dir);
        settled(s, &dir);
        let twin = Twin::of(s, &dir, &[".gitdupe", "docs/notes.md", "notes", "secret"]);
        for words in [
            &["-fd", "--", ":(exclude)docs/notes.md"][..],
            &["-fd", "--", ":(exclude)secret"],
            &["-fd", "--", ":(exclude)unrelated"],
            &["-fd", "--", "docs/*"],
            &["-fd", "--", "doc?"],
            // A pattern matching a hidden file's name spares it all the same.
            &["-fx", "--", "secre*", "scratch*"],
            &["-fdx", "--", ":!notes"],
        ] {
            twin.clean(words).run();
        }

        write(&dir, "docs/sub/notes.md", b"deeper\n");
        private_add(s, &dir, "docs/sub/notes.md");
        let before = Tree::working(&dir);
        let output = s
            .git(["dupe", "clean", "-fd", "--", ":(exclude)docs/sub/notes.md"])
            .from(&dir)
            .run();
        refused(
            &output,
            b":(exclude)docs/sub/notes.md",
            b"docs/sub",
            b"docs/sub/notes.md",
            false,
            &before,
            &dir,
        );
    });
}

/// The loose directories are read from the listing alone: a hidden path with nothing on
/// disk at its directory still lies below one, and so does a hidden path inside a nested
/// repository whose directory the project tracks nothing under, where the refusal is
/// over-inclusive (`Holds/G16`).
#[test]
fn a_loose_directory_is_found_whatever_stands_at_it() {
    under_each_release(|s| {
        let dir = project(s, "elsewhere");
        write(&dir, "scratch.txt", b"scratch\n");
        write(&dir, ".gitdupe", b"ghost/x\n");
        settled(s, &dir);
        let log = s.dir().join("trace");
        let before = Tree::working(&dir);
        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-n", "--", ":(exclude)unrelated"])
                .from(&dir),
            &log,
        );
        refused(
            &output,
            b":(exclude)unrelated",
            b"ghost",
            b"ghost/x",
            false,
            &before,
            &dir,
        );
        assert!(
            words_of(&runs, "ls-files")
                .iter()
                .any(|words| words.contains(&listed_under("ghost"))),
            "{runs:?}"
        );

        write(&dir, "nest/inner/secret5", b"secret\n");
        write(&dir, "nest/junk", b"junk\n");
        s.git(["init", "-q"])
            .from(&dir.join("nest/inner"))
            .succeeds();
        write(&dir, ".gitdupe", b"nest/inner/secret5\n");
        settled(s, &dir);
        let before = Tree::working(&dir);
        let output = s
            .git(["dupe", "clean", "-ffd", "--", ":(exclude)nest/inner"])
            .from(&dir)
            .run();
        refused(
            &output,
            b":(exclude)nest/inner",
            b"nest",
            b"nest/inner/secret5",
            false,
            &before,
            &dir,
        );
    });
}

/// The loose directories are those of the index Git's `clean` reads, the one a caller's
/// `GIT_INDEX_FILE` names, as a hook's does (S9, `Holds/G16`): a directory the public
/// index tracks a path in is loose in a named index that tracks nothing, where Git's
/// `clean` deletes it whole, and one the public index and a named index both track a path
/// in is not.
#[test]
fn the_loose_directories_are_those_of_the_index_the_clean_run_reads() {
    under_each_release(|s| {
        let dir = project(s, "named-index");
        write(&dir, "cache/public", b"public\n");
        s.git(["add", "--", "cache/public"]).from(&dir).succeeds();
        s.commit_public(&dir);
        s.git(["read-tree", "--empty"])
            .from(&dir)
            .variable("GIT_INDEX_FILE", dir.join(".git/empty-index"))
            .succeeds();
        s.git(["read-tree", "HEAD"])
            .from(&dir)
            .variable("GIT_INDEX_FILE", dir.join(".git/hook-index"))
            .succeeds();
        write(&dir, ".gitdupe", b"cache/secret\n");
        write(&dir, "cache/secret", b"secret\n");
        write(&dir, "cache/junk", b"junk\n");
        settled(s, &dir);

        let before = Tree::working(&dir);
        let output = s
            .git([
                "dupe",
                "clean",
                "-fdx",
                "--",
                "cache",
                ":(exclude)cache/secret",
            ])
            .from(&dir)
            .variable("GIT_INDEX_FILE", ".git/empty-index")
            .run();
        refused(
            &output,
            b":(exclude)cache/secret",
            b"cache",
            b"cache/secret",
            false,
            &before,
            &dir,
        );

        let twin = Twin::of(s, &dir, &[".gitdupe", "cache/secret"]);
        twin.clean(&["-fdx", "--", "cache", ":(exclude)cache/secret"])
            .variable("GIT_INDEX_FILE", ".git/hook-index")
            .run();
        twin.clean(&["-fdx", "--", "cache", ":(exclude)cache/secret"])
            .run();
    });
}

#[test]
fn the_help_text_names_the_refusal() {
    under_each_release(|s| {
        for words in [&["help", "clean"][..], &["clean", "-h"]] {
            let help = s.git(["dupe"].iter().chain(words)).run();
            assert_eq!(help.end, End::Code(0), "{help:?}");
            let text = String::from_utf8_lossy(&help.stdout);
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                text.contains(
                    "a pathspec holding *, ?, [, or \\, or beginning with : and not :/ \
                     alone, while one lies in a directory not hidden that holds nothing the \
                     project tracks"
                ),
                "{text}"
            );
        }
    });
}
