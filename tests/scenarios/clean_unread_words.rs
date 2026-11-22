//! A dashed word `git dupe clean` does not read (G26, F9): its usage error still names the
//! word and `git dupe git`, and, `clean` alone, also points at `git dupe help clean` for
//! the spellings it reads and says what `git dupe git clean` deletes, which is what that
//! Git's clean deletes in the private repository; `status` and `add` keep their line.

use crate::harness::{
    End, Output, Scenario, Tree, daily_state, names, unchanged, under_each_release, usage_line,
    write,
};

/// What the line and `git dupe help git` say `git dupe git clean` deletes.
const DELETES: &str = "does not track the project's files or private files never staged";

/// A command's text, through its own help.
fn command_text(s: &Scenario, command: &str) -> Vec<u8> {
    let output = s.git(["dupe", "help", command]).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
    output.stdout
}

/// The usage error's one line, after which comes the command's usage line, exit 129.
fn usage_error<'o>(output: &'o Output, text: &[u8]) -> &'o [u8] {
    assert_eq!(output.end, End::Code(129), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    output.line_then("error", usage_line(text))
}

#[test]
fn a_word_clean_does_not_read_names_its_help_and_what_the_unguarded_clean_deletes() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        daily_state(s, &root);
        write(&root, "scratch.txt", b"scratch\n");
        write(&root, "notes/draft.md", b"draft\n");
        let before = Tree::of(&root);
        let clean = command_text(s, "clean");
        for words in [
            ["clean", "--excl=dist/"].as_slice(),
            &["clean", "-fd", "--excl=dist/"],
            &["clean", "--excl=dist/", "-fdx"],
        ] {
            let output = s
                .git(["dupe"].into_iter().chain(words.iter().copied()))
                .from(&root)
                .run();
            let line = usage_error(&output, &clean);
            for part in [
                "'--excl=dist/' ",
                "git dupe git clean",
                "git dupe help clean",
                DELETES,
            ] {
                names(line, part.as_bytes());
            }
            unchanged(&before, &root);
        }
        // The other two tables keep the line they printed before: their unguarded route
        // deletes nothing.
        for (words, line) in [
            (
                ["status", "--porc"],
                "'--porc' is not an option git dupe status reads; git dupe git status gives \
                 it to Git unguarded",
            ),
            (
                ["add", "--pathspec-from-file=x"],
                "'--pathspec-from-file=x' is not an option git dupe add reads; git dupe git \
                 add gives it to Git unguarded",
            ),
        ] {
            let output = s.git(["dupe"].into_iter().chain(words)).from(&root).run();
            let text = command_text(s, words[0]);
            assert_eq!(usage_error(&output, &text), line.as_bytes(), "{output:?}");
        }
        unchanged(&before, &root);
    });
}

/// `git dupe git clean -f` deletes what the line says: a file the project tracks, edited,
/// and a private draft never staged, which are untracked in the private repository; a
/// privately tracked file stays. `git dupe help git` says the same.
#[test]
fn the_unguarded_clean_deletes_the_projects_files_and_private_drafts_never_staged() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        daily_state(s, &root);
        write(&root, "README.md", b"edited\n");
        write(&root, "notes/draft.md", b"draft\n");
        s.git(["dupe", "git", "clean", "-f"]).from(&root).succeeds();
        for gone in ["README.md", "docs/design.md", "notes/draft.md"] {
            assert!(!root.join(gone).exists(), "{gone} is still there");
        }
        for kept in ["notes/a.md", "docs/notes.md", ".gitdupe"] {
            assert!(root.join(kept).exists(), "{kept} is gone");
        }
        let help = s.git(["dupe", "help", "git"]).succeeds();
        let help = help
            .stdout
            .split(|&byte| byte == b'\n')
            .collect::<Vec<_>>()
            .join(&b' ');
        for part in ["git dupe git clean", DELETES, "git dupe help clean"] {
            names(&help, part.as_bytes());
        }
    });
}
