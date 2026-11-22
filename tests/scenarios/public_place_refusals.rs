//! What the refusal of a public place says (G18, `Holds/G18`, F8, F9): for its own
//! command, what would follow if Git took the place as the destination; where the
//! workspace is attached, `git dupe remote -v` to find the private remote; and `git dupe
//! git` for a destination the developer means. Each command a refusal names runs as the
//! refusal says from where it was refused, and none is named where it would be refused in
//! turn.

use std::fs;
use std::path::Path;

use crate::harness::{
    End, PROJECT_URL, Scenario, Transfer, Tree, as_git_spells, holds, names, refusal, unchanged,
    under_each_release,
};

const PUSH: &str = "push would send private history there";
const FETCH: &str = "fetch would bring the project's history into the private repository";
const PULL: &str = "pull would bring the project's history into the private repository and \
                    onto the branch checked out there";
const REMOTE: &str = "remote would configure it as a remote of the private repository";
const CLONE: &str = "clone takes the URL of the private repository, never the project's";

/// Every command's sentence, so that a line can be shown to hold its own alone.
const SENTENCES: [&str; 5] = [PUSH, FETCH, PULL, REMOTE, CLONE];

const REMOTES: &str = "'git dupe remote -v' lists the private remotes";
const UNGUARDED: &str = "'git dupe git' runs Git unguarded, for a destination you mean";
const UNGUARDED_ONCE_ATTACHED: &str =
    "'git dupe git' runs Git unguarded in an attached worktree, for a destination you mean";

/// Runs `git dupe <words>` from `from` and returns its refusal's line, which holds
/// `sentence` and no other command's, and `parts`; nothing in `t` changed.
fn refused(
    s: &Scenario,
    t: &Transfer,
    from: &Path,
    words: &[&str],
    sentence: &str,
    parts: &[&str],
) -> Vec<u8> {
    let before = t.everything();
    let output = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(from)
        .run();
    let line = refusal(&output, &[sentence.as_bytes()]).to_vec();
    for other in SENTENCES.iter().filter(|other| **other != sentence) {
        assert!(
            !holds(&line, other.as_bytes()),
            "{words:?}: {}",
            line.escape_ascii()
        );
    }
    for part in parts {
        names(&line, part.as_bytes());
    }
    before.unchanged();
    line
}

/// `git dupe remote -v` runs from the root as the refusals say: it lists the private
/// remote `origin`, the bare repository beside the root. Git prints a remote's URL as it
/// was configured, which `pushed_workspace` gave with this platform's separators.
fn lists_the_private_remotes(s: &Scenario, t: &Transfer) {
    let listed = s.git(["dupe", "remote", "-v"]).from(&t.root).succeeds();
    let private = t.private_remote.to_str().expect("a UTF-8 path");
    names(
        &listed.stdout,
        format!("origin\t{private} (push)").as_bytes(),
    );
}

#[test]
fn a_typed_word_gets_its_own_commands_sentence_and_the_private_remotes() {
    under_each_release(|s| {
        let t = s.pushed_workspace("workspace");
        let sub = t.root.join("sub");
        fs::create_dir(&sub).unwrap();
        let root = as_git_spells(&t.root);
        let quoted = format!("'{PROJECT_URL}' names a URL of the project's remote 'origin'; ");
        let cases: [(&[&str], &str); 5] = [
            (&["push", PROJECT_URL, "main"], PUSH),
            (&["fetch", PROJECT_URL], FETCH),
            (&["pull", PROJECT_URL, "main"], PULL),
            (&["remote", "add", "leak", PROJECT_URL], REMOTE),
            (&["remote", "set-url", "origin", PROJECT_URL], REMOTE),
        ];
        for (words, sentence) in cases {
            let line = refused(s, &t, &t.root, words, sentence, &[REMOTES, UNGUARDED]);
            let expected = format!(
                "{quoted}if Git took it as the destination, {sentence}; {REMOTES}, and {UNGUARDED}"
            );
            assert_eq!(line, expected.as_bytes(), "{words:?}");
        }
        // A path from a subdirectory, the root's own name for it.
        let line = refused(s, &t, &sub, &["push", ".", "main:leak"], PUSH, &[REMOTES]);
        names(
            &line,
            format!("the project's working tree '{root}'").as_bytes(),
        );
        lists_the_private_remotes(s, &t);
    });
}

/// An option's value, an empty one included, is compared wherever it stands, so the
/// sentence says what would follow, never what the command was doing.
#[test]
fn an_options_value_gets_the_sentence_of_what_would_follow() {
    under_each_release(|s| {
        let t = s.pushed_workspace("workspace");
        let root = as_git_spells(&t.root);
        let repo = format!("--repo={PROJECT_URL}");
        let cases: [(&[&str], &str, &str); 3] = [
            (
                &["fetch", "origin", "--refmap="],
                FETCH,
                "'--refmap=' names",
            ),
            (
                &["push", &repo],
                PUSH,
                "a URL of the project's remote 'origin'",
            ),
            (
                &["pull", "--upload-pack=.", "origin"],
                PULL,
                "'--upload-pack=.' names",
            ),
        ];
        for (words, sentence, place) in cases {
            let line = refused(
                s,
                &t,
                &t.root,
                words,
                sentence,
                &[place, REMOTES, UNGUARDED],
            );
            names(&line, b"if Git took it as the destination, ");
            if words[0] != "push" {
                names(
                    &line,
                    format!("the project's working tree '{root}'").as_bytes(),
                );
            }
        }
        lists_the_private_remotes(s, &t);
    });
}

/// Given no repository, the remote Git may take in its place is refused with the
/// command's own sentence, said of what would follow if Git took it.
#[test]
fn the_default_remote_gets_its_commands_sentence_and_the_private_remotes() {
    under_each_release(|s| {
        let t = s.pushed_workspace("workspace");
        s.private(&t.root)
            .git(["config", "branch.main.remote", PROJECT_URL])
            .succeeds();
        for (command, sentence) in [("push", PUSH), ("fetch", FETCH), ("pull", PULL)] {
            let line = refused(s, &t, &t.root, &[command], sentence, &[REMOTES, UNGUARDED]);
            let expected = format!(
                "{command} given no repository may take '{PROJECT_URL}' from \
                 branch.main.remote, which names a URL of the project's remote 'origin'; if \
                 Git took it as the destination, {sentence}; {REMOTES}, and {UNGUARDED}"
            );
            assert_eq!(line, expected.as_bytes(), "{command}");
        }
        lists_the_private_remotes(s, &t);
    });
}

/// Where no workspace is attached, as in the linked worktree no `init` or `clone` has
/// attached, `git dupe remote -v` and `git dupe git` are themselves refused, so a refusal
/// there names no remote listing and says that `git dupe git` needs an attached worktree;
/// a help request among the words is compared there too.
#[test]
fn where_nothing_is_attached_no_command_is_named_as_running_there() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let cases: [(&[&str], &str); 3] = [
            (&["push", PROJECT_URL, "-h"], PUSH),
            (&["fetch", "--help", PROJECT_URL], FETCH),
            (&["remote", "add", "leak", PROJECT_URL, "-h"], REMOTE),
        ];
        for (words, sentence) in cases {
            let line = refused(
                s,
                &t,
                &t.linked,
                words,
                sentence,
                &[UNGUARDED_ONCE_ATTACHED],
            );
            assert!(
                !holds(&line, b"git dupe remote"),
                "{words:?}: {}",
                line.escape_ascii()
            );
        }
        for words in [["dupe", "remote", "-v"], ["dupe", "git", "remote"]] {
            let unattached = s.git(words).from(&t.linked).run();
            assert_eq!(unattached.end, End::Code(128), "{unattached:?}");
        }
    });
}

/// `clone`'s refusal comes where no workspace is attached: it says what `clone` takes,
/// for its URL and its branch word alike, and names no command but `git dupe git`, said
/// to need an attached worktree.
#[test]
fn clone_says_it_takes_the_private_repositorys_url_and_adds_no_command() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let second = &m.root;
        let url = m.first.private_remote.to_str().unwrap();
        // The second machine's public remote `origin` is the first machine's root.
        let public = m.first.root.to_str().unwrap();
        let before = Tree::of(second);
        for words in [
            ["dupe", "clone", public].as_slice(),
            &["dupe", "clone", url, "-b", "."],
        ] {
            let output = s.git(words).from(second).run();
            let line = refusal(
                &output,
                &[CLONE.as_bytes(), UNGUARDED_ONCE_ATTACHED.as_bytes()],
            );
            for other in [PUSH, FETCH, PULL, REMOTE, REMOTES] {
                assert!(!holds(line, other.as_bytes()), "{}", line.escape_ascii());
            }
            let commands = line.windows(9).filter(|part| *part == b"git dupe ").count();
            assert_eq!(commands, 1, "{}", line.escape_ascii());
            names(line, b"; clone takes the URL");
        }
        assert!(!second.join(".git/dupe").exists());
        unchanged(&before, second);
    });
}

/// The refusals whose next step removes a private remote or group keep their line: what
/// names the place there is the private configuration, which that step repairs.
#[test]
fn the_removing_variants_keep_their_line() {
    under_each_release(|s| {
        let t = s.pushed_workspace("workspace");
        s.git(["dupe", "git", "remote", "add", "leak", PROJECT_URL])
            .from(&t.root)
            .succeeds();
        let output = s.git(["dupe", "push", "leak", "main"]).from(&t.root).run();
        let line = refusal(&output, &[]);
        assert_eq!(
            line,
            b"a URL of the private remote 'leak' names a URL of the project's remote 'origin'; \
              'git dupe remote remove' removes that remote",
        );
        s.git(["dupe", "git", "remote", "remove", "leak"])
            .from(&t.root)
            .succeeds();
        s.git([
            "dupe",
            "config",
            "remotes.both",
            &format!("origin {PROJECT_URL}"),
        ])
        .from(&t.root)
        .succeeds();
        let output = s.git(["dupe", "fetch", "origin"]).from(&t.root).run();
        let line = refusal(&output, &[]);
        assert_eq!(
            line,
            b"a member of the private remote group 'remotes.both' names a URL of the \
              project's remote 'origin'; 'git dupe config --unset-all remotes.both' removes \
              that group",
        );
    });
}
