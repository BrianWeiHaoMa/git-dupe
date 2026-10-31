//! What Windows alone gives git-dupe to meet: a received `Git_Dir`, which is `GIT_DIR` there
//! (G10, S20); a name that is not valid UTF-8, which an index or a `.gitdupe` can hold and a
//! command line cannot (G30); a junction, which counts as a symbolic link (G6, G16, G27, S24);
//! and a file held without delete sharing, which stops a removal partway (`Holds/G3`). Off
//! Windows none of these is what it is here, so each check reports itself unavailable there.

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, held_from_deletion, holds, junction, linked_on_branch, names,
    private_add, private_commit, privately_tracked, region, region_rules, report,
    under_each_release, word, write,
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
