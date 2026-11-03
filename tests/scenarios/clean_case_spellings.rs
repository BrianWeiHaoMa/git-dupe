//! What `git dupe clean -X` does where Git enters a directory git-dupe's bytes do not name:
//! a hidden directory spelled in another ASCII case on a volume that equates spellings, and
//! a junction on Windows (G16, the sixth and seventh limits, S4, S18, S24). Git enters a
//! directory under which the index holds a path, in any spelling, so that a pattern
//! keeping the directory out would make everything below it ignored; the listing is asked
//! in every spelling of its case, and one entry it reaches that way alone keeps no
//! directory out at all.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, equates_spellings, holds, junction, private_add, report, symlink,
    under_each_release, word, write,
};

/// Whether the volume this scenario needs is absent here, which it then reports.
fn no_such_volume(s: &Scenario) -> bool {
    if word() == "linux" {
        report(
            "clean_case_spellings unavailable: it checks a volume that equates spellings, on \
             macOS and on Windows alone",
        );
        return true;
    }
    if !equates_spellings(s.dir()) {
        report(
            "clean_case_spellings unavailable: the scenarios' directory distinguishes case, \
             where macOS's default volume and NTFS do not",
        );
        return true;
    }
    false
}

/// A project that ignores `*.log`, hiding `notes` before anything stands at it. On a volume
/// that equates spellings `Notes` is then that directory, whatever the path's own spelling.
fn hiding_notes(s: &Scenario, name: &str) -> PathBuf {
    let root = s.dir().join(name);
    s.attached_project(&root);
    write(
        &root,
        ".gitignore",
        b".env.local\n.vscode/\nbuild/\n*.log\n",
    );
    s.git(["add", "--", ".gitignore"]).from(&root).succeeds();
    s.commit_public(&root);
    s.git(["dupe", "hide", "notes"]).from(&root).succeeds();
    root
}

/// `path` made public with `-f`, as a file reaches a hidden directory's other spelling.
fn public_file(s: &Scenario, root: &Path, path: &str) {
    write(root, path, b"public\n");
    s.git(["add", "-f", "--", path]).from(root).succeeds();
    s.commit_public(root);
}

/// `git dupe clean WORDS` from the root, exit 0 and no `warning:`.
fn cleaned(s: &Scenario, root: &Path, words: &[&str]) -> Output {
    let command = ["dupe", "clean"].iter().chain(words);
    let output = s.git(command).from(root).run();
    assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
    assert!(output.lines("warning").is_empty(), "{words:?}: {output:?}");
    output
}

fn stands(root: &Path, path: &str, content: &[u8], words: &[&str]) {
    let read = fs::read(root.join(path));
    assert_eq!(
        read.ok().as_deref(),
        Some(content),
        "{path} after {words:?}"
    );
}

fn gone(root: &Path, path: &str, words: &[&str]) {
    assert!(!root.join(path).exists(), "{path} after {words:?}");
}

#[test]
fn a_hidden_directory_the_project_tracks_a_file_in_under_another_spelling_keeps_its_files() {
    under_each_release(|s| {
        if no_such_volume(s) {
            return;
        }
        let root = hiding_notes(s, "project");
        public_file(s, &root, "Notes/README.md");
        write(&root, "Notes/draft.md", b"draft\n");

        let asked = cleaned(s, &root, &["-nX"]);
        assert!(
            !holds(&asked.stdout.to_ascii_lowercase(), b"notes"),
            "{asked:?}"
        );
        for words in [
            &["-fX"][..],
            &["-fx"],
            &["-fdX"],
            &["-fdx"],
            &["-fX", "--", "Notes"],
            &["-fx", "--", "Notes/draft.md"],
        ] {
            cleaned(s, &root, words);
            stands(&root, "Notes/draft.md", b"draft\n", words);
            stands(&root, "Notes/README.md", b"public\n", words);
        }

        // A staged file edited afterwards keeps the edit.
        let staged = s.git(["dupe", "add", "--", "Notes/draft.md"]);
        staged.from(&root).succeeds();
        write(&root, "Notes/draft.md", b"edited\n");
        cleaned(s, &root, &["-fX"]);
        stands(&root, "Notes/draft.md", b"edited\n", &["-fX"]);
    });
}

#[test]
fn everything_below_the_directory_is_spared_and_what_lies_elsewhere_is_not() {
    under_each_release(|s| {
        if no_such_volume(s) {
            return;
        }
        let root = hiding_notes(s, "project");
        public_file(s, &root, "Notes/README.md");
        write(&root, "Notes/draft.md", b"draft\n");
        write(&root, "Notes/junk.log", b"ignored\n");
        write(&root, "out.log", b"ignored\n");

        let words = &["-fX"];
        cleaned(s, &root, words);
        gone(&root, "out.log", words);
        for (path, content) in [
            ("Notes/draft.md", &b"draft\n"[..]),
            ("Notes/junk.log", b"ignored\n"),
            ("Notes/README.md", b"public\n"),
        ] {
            stands(&root, path, content, words);
        }
    });
}

/// One entry reached through its case withholds the pattern from every spared path: the
/// bytes cannot say which directory it lies under, and a build that saw `other/README.md`
/// covered by its bytes and kept the pattern would lose `Notes/draft.md`.
#[test]
fn one_entry_reached_through_its_case_keeps_no_directory_out_at_all() {
    under_each_release(|s| {
        if no_such_volume(s) {
            return;
        }
        let root = hiding_notes(s, "project");
        s.git(["dupe", "hide", "other"]).from(&root).succeeds();
        public_file(s, &root, "Notes/README.md");
        public_file(s, &root, "other/README.md");
        write(&root, "Notes/draft.md", b"draft\n");
        write(&root, "other/draft.md", b"draft\n");

        let words = &["-fX"];
        cleaned(s, &root, words);
        for path in ["Notes/draft.md", "other/draft.md"] {
            stands(&root, path, b"draft\n", words);
        }
    });
}

/// A directory holding a `.git` entry above the hidden directory, under which the project
/// tracks the path in another spelling: the listing is asked about that ancestor too, and
/// what it names through the ancestor's bytes still reaches the hidden directory.
#[test]
fn an_ancestor_holding_a_git_entry_hides_no_entry_reached_through_its_case() {
    under_each_release(|s| {
        if no_such_volume(s) {
            return;
        }
        let root = hiding_notes(s, "project");
        s.git(["dupe", "hide", "outer/notes"])
            .from(&root)
            .succeeds();
        fs::create_dir_all(root.join("outer/.git")).unwrap();
        public_file(s, &root, "outer/Notes/README");
        write(&root, "outer/Notes/private", b"private\n");

        let words = &["-fX"];
        cleaned(s, &root, words);
        stands(&root, "outer/Notes/private", b"private\n", words);
    });
}

/// G16's corner, on a volume of any kind: while the project tracks a path that, read
/// without regard to its case, lies at or below a hidden directory, a directory at a hidden
/// path is no one entry to Git, so that the ignored file beside it in a directory no rule
/// names goes where plain `git clean -X` keeps it. Where spellings are told apart `Notes`
/// is another directory, which the listing names the same way.
#[test]
fn an_entry_reached_through_its_case_gives_up_the_directory_kept_whole_beside_a_hidden_one() {
    under_each_release(|s| {
        let root = hiding_notes(s, "project");
        s.git(["dupe", "hide", "tmp/notes"]).from(&root).succeeds();
        write(&root, "notes/draft.md", b"draft\n");
        write(&root, "tmp/notes/secret", b"secret\n");
        write(&root, "tmp/junk.log", b"ignored\n");

        let words = &["-fX"];
        cleaned(s, &root, words);
        stands(&root, "tmp/junk.log", b"ignored\n", words);

        public_file(s, &root, "Notes/README.md");
        cleaned(s, &root, words);
        gone(&root, "tmp/junk.log", words);
        stands(&root, "tmp/notes/secret", b"secret\n", words);
        stands(&root, "notes/draft.md", b"draft\n", words);
        stands(&root, "Notes/README.md", b"public\n", words);
    });
}

/// A junction at a hidden path, under which the index holds a path: `lstat` calls it a link
/// and Git walks it (S24), so the listing is asked about it as about a directory.
#[test]
fn a_junction_at_a_hidden_path_keeps_what_stands_in_it() {
    under_each_release(|s| {
        if word() != "windows" {
            report("clean_case_spellings unavailable: it checks a junction, on Windows alone");
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        s.git(["dupe", "hide", "notes", "backing"])
            .from(&root)
            .succeeds();
        write(&root, "backing/draft.md", b"draft\n");
        write(&root, "backing/README.md", b"public\n");
        junction(&root.join("backing"), &root.join("notes"));
        let forced = s.git(["add", "-f", "--", "notes/README.md"]);
        forced.from(&root).succeeds();
        write(&root, "scratch.txt", b"scratch\n");

        for words in [&["-fX"][..], &["-fdX"], &["-fx"], &["-fdx"]] {
            cleaned(s, &root, words);
            stands(&root, "backing/draft.md", b"draft\n", words);
            stands(&root, "backing/README.md", b"public\n", words);
        }
        // Removed here, because no cleanup of the scenario's directory may walk it.
        fs::remove_dir(root.join("notes")).unwrap();
    });
}

/// A symbolic link at a hidden path, not above one: Git's `clean` does not enter it, and the
/// listing's answer for it changes nothing that stands (G16).
#[test]
fn a_symbolic_link_at_a_hidden_path_is_kept_with_what_it_leads_to() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        s.attached_project(&root);
        s.git(["dupe", "hide", "notes", "backing"])
            .from(&root)
            .succeeds();
        write(&root, "backing/draft.md", b"draft\n");
        private_add(s, &root, "backing/draft.md");
        public_file(s, &root, "backing/README.md");
        symlink("backing", root.join("notes")).unwrap();

        for words in [&["-fX"][..], &["-fdX"]] {
            cleaned(s, &root, words);
            let link = fs::symlink_metadata(root.join("notes"));
            assert!(link.unwrap().file_type().is_symlink(), "after {words:?}");
            stands(&root, "backing/draft.md", b"draft\n", words);
            stands(&root, "backing/README.md", b"public\n", words);
        }
    });
}
