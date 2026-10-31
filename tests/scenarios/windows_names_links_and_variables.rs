//! What Windows alone gives git-dupe to meet: a received `Git_Dir`, which is `GIT_DIR` there
//! (G10, S20); a name that is not valid UTF-8, which an index or a `.gitdupe` can hold and a
//! command line cannot (G30); a junction, which counts as a symbolic link (G6, G16, G27, S24);
//! a file held without delete sharing, which stops a removal partway (`Holds/G3`); a read-only
//! destination, which refuses a replacement for good (S22); and a path no bracket expression
//! can stand on (the seventh limit, G12). Off Windows none of these is what it is here, so each
//! check reports itself unavailable there.

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, as_git_spells, entry_names, held_from_deletion, holds, junction,
    linked_on_branch, names, names_path, private_add, private_commit, privately_tracked, region,
    region_rules, report, under_each_release, word, write,
};

/// A name no command line here can carry.
const NO_NAME: &[u8] = b"a\xffb";

fn unavailable_here() -> bool {
    let unavailable = word() != "windows";
    if unavailable {
        report("windows_names_links_and_variables unavailable: it checks Windows, on Windows");
    }
    unavailable
}

/// The `warning:` lines of `output` that hold `part`.
fn warned_of<'o>(output: &'o Output, part: &[u8]) -> Vec<&'o [u8]> {
    let warnings = output.lines("warning");
    warnings
        .into_iter()
        .filter(|line| holds(line, part))
        .collect()
}

/// The one `fatal:` line of `output`, which a `warning:` may follow.
fn the_refusal(output: &Output) -> &[u8] {
    let fatal = output.lines("fatal");
    assert_eq!(fatal.len(), 1, "{output:?}");
    fatal[0]
}

/// `status` refuses over `NO_NAME`, naming it, and warns that it was not asked about.
fn no_name_refused(s: &Scenario, root: &Path) {
    let index = s.private(root).git(["ls-files", "-s"]).succeeds().stdout;
    let listed = fs::read(root.join(".gitdupe"));
    let status = s.git(["dupe", "status"]).from(root).run();
    assert_eq!(status.end, End::Code(128), "{status:?}");
    names(the_refusal(&status), NO_NAME);
    assert_eq!(warned_of(&status, NO_NAME).len(), 1, "{status:?}");
    assert_eq!(
        s.private(root).git(["ls-files", "-s"]).succeeds().stdout,
        index
    );
    assert_eq!(fs::read(root.join(".gitdupe")).ok(), listed.ok());
    // The rule is bytes in a file, which no command line carries.
    let rule = [b"/", NO_NAME].concat();
    assert!(
        region_rules(root).contains(&rule),
        "{:?}",
        region_rules(root)
    );
}

#[test]
fn a_received_git_dir_in_any_case_is_the_private_repositorys_to_replace() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        write(&root, "notes/a.md", b"private\n");
        s.git(["dupe", "hide", "notes"]).from(&root).succeeds();
        let public_index = s.git(["ls-files", "-s"]).from(&root).succeeds().stdout;

        let added = s
            .git(["dupe", "add", "notes/a.md"])
            .from(&root)
            .variable("Git_Dir", root.join(".git"))
            .succeeds();
        assert!(added.lines("fatal").is_empty(), "{added:?}");
        assert!(privately_tracked(s, &root).contains(&b"notes/a.md".to_vec()));
        assert_eq!(
            s.git(["ls-files", "-s"]).from(&root).succeeds().stdout,
            public_index
        );
        let status = s
            .git(["dupe", "status", "--porcelain"])
            .from(&root)
            .variable("Git_Dir", root.join(".git"))
            .succeeds();
        assert!(holds(&status.stdout, b"A  notes/a.md"), "{status:?}");
    });
}

#[test]
fn a_privately_tracked_name_that_is_not_utf_8_is_refused_and_warned_of() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        let blob = s
            .private(&root)
            .git(["hash-object", "-w", "--stdin"])
            .input(b"bytes\n")
            .succeeds();
        let hash = blob.stdout.trim_ascii();
        let entry = [b"100644 ", hash, b" 0\t", NO_NAME, b"\0"].concat();
        s.private(&root)
            .git(["update-index", "-z", "--index-info"])
            .input(&entry)
            .succeeds();
        no_name_refused(s, &root);
    });
}

#[test]
fn a_listed_name_that_is_not_utf_8_is_refused_and_warned_of_by_a_command_it_is_no_word_of() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        write(&root, "notes/a.md", b"private\n");
        s.git(["dupe", "add", "notes"]).from(&root).succeeds();
        private_add(s, &root, ".gitdupe");
        private_commit(s, &root);
        write(&root, ".gitdupe", &[b"notes\n", NO_NAME, b"\n"].concat());
        no_name_refused(s, &root);

        // Only that path goes unasked: the question still reaches the path beside it.
        write(&root, ".gitignore", b"!notes\n");
        let log = s.git(["dupe", "log", "-1"]).from(&root).succeeds();
        assert_eq!(warned_of(&log, NO_NAME).len(), 1, "{log:?}");
        let exposed = warned_of(&log, b"notes is hidden");
        assert_eq!(exposed.len(), 1, "{log:?}");
        names(exposed[0], b".gitignore");
        assert!(
            warned_of(&log, b"exposure was not checked").is_empty(),
            "{log:?}"
        );
    });
}

#[test]
fn a_listed_name_that_is_not_utf_8_is_refused_by_add_and_clean_and_named_by_detach() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        write(&root, "notes/a.md", b"private\n");
        s.git(["dupe", "add", "notes"]).from(&root).succeeds();
        let listed = [b"notes\n", NO_NAME, b"\n"].concat();
        write(&root, ".gitdupe", &listed);
        write(&root, "scratch.txt", b"scratch\n");
        let index = s.private(&root).git(["ls-files", "-s"]).succeeds().stdout;

        for words in [&["dupe", "add", "-A"][..], &["dupe", "clean", "-fdx"]] {
            let refused = s.git(words).from(&root).run();
            assert_eq!(refused.end, End::Code(128), "{words:?}: {refused:?}");
            names(the_refusal(&refused), NO_NAME);
            assert_eq!(
                s.private(&root).git(["ls-files", "-s"]).succeeds().stdout,
                index
            );
            assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), listed);
            assert!(root.join("scratch.txt").exists(), "{words:?} deleted it");
        }

        // `detach` asks about the path beside it, and names this one instead of refusing.
        let detached = s.git(["dupe", "detach", "--force"]).from(&root).run();
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        assert_eq!(warned_of(&detached, NO_NAME).len(), 1, "{detached:?}");
        assert!(!root.join(".git").join("dupe").exists());
    });
}

#[test]
fn a_hidden_path_below_a_junction_is_warned_of_and_clean_refuses() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        let outside = s.dir().join("outside");
        write(&outside, "notes/x", b"beyond the junction\n");
        junction(&outside, &root.join("link"));
        write(&root, "scratch.txt", b"scratch\n");
        s.git(["dupe", "hide", "link/notes"]).from(&root).succeeds();

        let status = s.git(["dupe", "status"]).from(&root).succeeds();
        assert_eq!(warned_of(&status, b"link/notes").len(), 1, "{status:?}");

        let clean = s.git(["dupe", "clean", "-fdx"]).from(&root).run();
        assert_eq!(clean.end, End::Code(128), "{clean:?}");
        let fatal = the_refusal(&clean);
        names(fatal, b"link/notes");
        names(fatal, b"link ");
        assert!(root.join("scratch.txt").exists(), "clean deleted nothing");
        assert!(outside.join("notes/x").exists(), "clean deleted nothing");
    });
}

#[test]
fn a_path_of_another_worktrees_region_below_a_junction_is_not_warned_of() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let main = s.dir().join("project");
        s.attached_project(&main);
        let linked = s.dir().join("linked");
        linked_on_branch(s, &main, &linked, "agent");
        s.init(&linked);
        s.git(["dupe", "hide", "sub/foreign"])
            .from(&linked)
            .succeeds();

        // A plain directory there gets the warning a junction does not.
        write(&main, "sub/foreign/x", b"here\n");
        let beside = s.git(["dupe", "status"]).from(&main).succeeds();
        assert_eq!(warned_of(&beside, b"sub/foreign").len(), 1, "{beside:?}");

        fs::remove_dir_all(main.join("sub")).unwrap();
        let outside = s.dir().join("outside");
        write(&outside, "foreign/x", b"beyond the junction\n");
        junction(&outside, &main.join("sub"));
        let below = s.git(["dupe", "status"]).from(&main).succeeds();
        assert!(warned_of(&below, b"sub/foreign").is_empty(), "{below:?}");
    });
}

/// A junction counts as a symbolic link wherever one is named (G6): at `.git/info` the region
/// is left as it is, and `detach` refuses (G3).
#[test]
fn a_junction_at_git_info_is_a_link_to_the_region_and_to_detach() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        s.git(["dupe", "status"]).from(&root).succeeds();
        let info = root.join(".git").join("info");
        let moved = s.dir().join("moved-info");
        fs::rename(&info, &moved).unwrap();
        junction(&moved, &info);
        write(&root, ".gitdupe", b"notes\n");
        let exclude = fs::read(moved.join("exclude")).unwrap();

        let status = s.git(["dupe", "status"]).from(&root).succeeds();
        let warned = warned_of(&status, b"cannot be maintained");
        assert_eq!(warned.len(), 1, "{status:?}");
        names_path(warned[0], as_git_spells(&info).as_bytes());
        assert_eq!(fs::read(moved.join("exclude")).unwrap(), exclude);

        let refused = s.git(["dupe", "detach", "--force"]).from(&root).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        names_path(the_refusal(&refused), as_git_spells(&info).as_bytes());
        assert!(root.join(".git").join("dupe").is_dir());
        assert_eq!(fs::read(moved.join("exclude")).unwrap(), exclude);
    });
}

fn read_only(path: &Path, read_only: bool) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(read_only);
    fs::set_permissions(path, permissions).unwrap();
}

/// What no `RLIMIT_FSIZE` can make fail here a read-only destination does: past the bounded
/// retry the replacement is refused for good, and every route of `State` meets that refusal.
#[test]
fn a_read_only_destination_fails_each_replacement_as_a_failed_write_does() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        s.git(["dupe", "status"]).from(&root).succeeds();
        let exclude = root.join(".git").join("info").join("exclude");
        let private = root.join(".git").join("dupe");
        let settled = fs::read(&exclude).unwrap();
        let private_before = entry_names(&private);
        write(&root, ".gitdupe", b"notes\n");
        read_only(&exclude, true);

        // The region: a warning naming the file, the command's status kept.
        let status = s.git(["dupe", "log", "-1"]).from(&root).run();
        assert_eq!(
            status.end,
            s.private(&root).git(["log", "-1"]).run().end,
            "{status:?}"
        );
        let refused = warned_of(&status, b"cannot replace the managed region");
        assert_eq!(refused.len(), 1, "{status:?}");
        names_path(refused[0], as_git_spells(&exclude).as_bytes());
        assert_eq!(fs::read(&exclude).unwrap(), settled);
        assert_eq!(entry_names(&private), private_before);

        // `detach`, whose first step this is: refused, with the region and the repository.
        let before = Tree::of(&private);
        let detach = s.git(["dupe", "detach", "--force"]).from(&root).run();
        assert_eq!(detach.end, End::Code(128), "{detach:?}");
        names_path(the_refusal(&detach), as_git_spells(&exclude).as_bytes());
        assert_eq!(fs::read(&exclude).unwrap(), settled);
        assert!(before.changed_in(&Tree::of(&private)).is_empty());

        // Then the next command writes the region as it would have, read-only as it stood.
        read_only(&exclude, false);
        s.git(["dupe", "status"]).from(&root).succeeds();
        assert_eq!(region_rules(&root), [&b"/.gitdupe"[..], b"/notes"]);

        // `.gitdupe`: the command that asked fails, naming the cause, and nothing else moves.
        read_only(&root.join(".gitdupe"), true);
        let index = s.private(&root).git(["ls-files", "-s"]).succeeds().stdout;
        let hide = s.git(["dupe", "hide", "scratch"]).from(&root).run();
        assert_eq!(hide.end, End::Code(128), "{hide:?}");
        names(the_refusal(&hide), b".gitdupe");
        assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"notes\n");
        assert_eq!(
            s.private(&root).git(["ls-files", "-s"]).succeeds().stdout,
            index
        );
        read_only(&root.join(".gitdupe"), false);
        s.git(["dupe", "hide", "scratch"]).from(&root).succeeds();
    });
}

/// A publicly tracked path under a hidden path is left out by a pattern standing on its last
/// ASCII byte; with none to stand on it is left out with what lies below it (the seventh limit).
#[test]
fn a_public_path_with_no_ascii_byte_is_left_out_with_what_stands_below_it() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.repository(&root);
        write(&root, "é/ü", b"public\n");
        write(&root, "é/a", b"public\n");
        s.git(["add", "-A"]).from(&root).succeeds();
        s.commit_public(&root);
        s.init(&root);
        write(&root, ".gitdupe", "é\n".as_bytes());
        // A directory where the public index holds each file, a file standing below it.
        for public in ["é/ü", "é/a"] {
            fs::remove_file(root.join(public)).unwrap();
            write(&root, &format!("{public}/below"), b"private\n");
        }

        let status = s
            .git([
                "dupe",
                "status",
                "--porcelain",
                "-z",
                "--untracked-files=all",
            ])
            .from(&root)
            .succeeds();
        assert!(holds(&status.stdout, "é/a/below".as_bytes()), "{status:?}");
        assert!(!holds(&status.stdout, "é/ü".as_bytes()), "{status:?}");
    });
}

/// Git's objects are held by no one: a file of the private repository this check holds.
#[test]
fn a_held_file_stops_detach_partway_and_force_completes_it() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        s.git(["dupe", "status"]).from(&root).succeeds();
        let held = root.join(".git").join("dupe").join("held");
        fs::write(&held, b"held\n").unwrap();
        let holder = held_from_deletion(&held);

        let stopped = s.git(["dupe", "detach", "--force"]).from(&root).run();
        assert_eq!(stopped.end, End::Code(128), "{stopped:?}");
        names(the_refusal(&stopped), b"dupe");
        assert!(held.exists(), "the held file stands");
        assert!(region(&root).is_none(), "the region went first");

        drop(holder);
        s.git(["dupe", "detach", "--force"]).from(&root).succeeds();
        assert!(!root.join(".git").join("dupe").exists());
    });
}
