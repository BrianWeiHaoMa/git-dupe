//! Done when's ordinary linked-worktree story and In use 11: clone, foreign hidden
//! files, adoption, private push/merge, and removal followed by stale-region settlement
//! meet the state their preceding commands left (G2, G27, F2, F3, S17).

use std::fs;

use crate::harness::{
    Scenario, Tree, Worktree, records, under_each_release, worktree_clone, worktree_dupe,
    worktree_private_head, worktree_public_clean, write,
};

const NOTES_WARNING: &[u8] = b"notes stands here and is hidden by the main worktree alone: public Git ignores it here, and this worktree does not hide it; run from the root, 'git dupe hide -- notes' hides it here too";
const SCRATCH_WARNING: &[u8] = b"scratch stands here and is hidden by the main worktree alone: public Git ignores it here, and this worktree does not hide it; run from the root, 'git dupe hide -- scratch' hides it here too";
const TOP: &[u8] = b"# user's rules\n*.orig\n";
const BETWEEN: &[u8] = b"# user's text between worktrees\n";
const LAST: &[u8] = b"# user's last text without newline";

fn commit(s: &Scenario, wt: &Worktree, message: &str) {
    worktree_dupe(s, wt, &["commit", "-m", message]);
}

#[test]
fn linked_private_work_is_adopted_shared_and_removed_without_public_changes() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        let main = Worktree::read(s, &root);
        write(&root, "notes/plan.md", b"private plan\n");
        worktree_dupe(s, &main, &["add", "notes/"]);
        commit(s, &main, "Private notes");
        let main_region = main.region_bytes();
        let exclude = main.common_directory.join("info/exclude");
        fs::write(&exclude, [TOP, &main_region, BETWEEN].concat()).unwrap();
        s.git(["worktree", "add", "../project-agent", "-b", "agent"])
            .from(&root)
            .succeeds();
        let agent = Worktree::read(s, &s.dir().join("project-agent"));
        assert_eq!(
            agent.private_directory(),
            root.join(".git/worktrees/project-agent/dupe")
        );
        let source = Tree::of(&main.private_directory());
        worktree_clone(s, &agent, &main.private_directory());
        assert!(
            source
                .changed_in(&Tree::of(&main.private_directory()))
                .is_empty()
        );
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(
            worktree_private_head(s, &agent),
            worktree_private_head(s, &main)
        );
        assert_eq!(
            agent
                .private(s)
                .git(["ls-files", "-s", "-z"])
                .succeeds()
                .stdout,
            main.private(s)
                .git(["ls-files", "-s", "-z"])
                .succeeds()
                .stdout
        );
        for path in [".gitdupe", "notes/plan.md"] {
            assert_eq!(
                fs::read(agent.root.join(path)).unwrap(),
                fs::read(root.join(path)).unwrap()
            );
        }
        assert_eq!(
            agent.region().unwrap().rules,
            [b"/.gitdupe".to_vec(), b"/notes".to_vec()]
        );
        assert_eq!(agent.region_bytes(), b"# BEGIN git-dupe worktree project-agent\n/.gitdupe\n/notes\n# END git-dupe worktree project-agent\n");
        worktree_public_clean(s, &agent);

        write(&agent.root, "notes/x.md", b"never privately staged\n");
        let tracked = agent.private(s).git(["ls-files", "-z"]).succeeds();
        assert_eq!(
            records(&tracked.stdout, 0),
            [b".gitdupe".as_slice(), b"notes/plan.md"]
        );
        worktree_dupe(s, &agent, &["unhide", "notes/"]);
        worktree_public_clean(s, &agent);
        let warned = worktree_dupe(s, &agent, &["status", "--porcelain"]);
        assert_eq!(warned.lines("warning"), [NOTES_WARNING], "{warned:?}");
        assert_eq!(warned.stdout, b"M  .gitdupe\n", "{warned:?}");
        assert_eq!(fs::read(agent.root.join(".gitdupe")).unwrap(), b"");
        assert_eq!(
            agent
                .private(s)
                .git(["cat-file", "blob", ":.gitdupe"])
                .succeeds()
                .stdout,
            b""
        );
        assert_eq!(
            agent.region().unwrap().rules,
            [b"/.gitdupe".to_vec(), b"/notes/plan.md".to_vec()]
        );
        assert_eq!(
            fs::read(agent.root.join("notes/x.md")).unwrap(),
            b"never privately staged\n"
        );

        // Restore notes before committing, so its unhide cannot remove main's notes
        // declaration on merge. Commit main's later scratch declaration before merge:
        // hide stages .gitdupe, and that staged change would otherwise block it.
        worktree_dupe(s, &agent, &["hide", "notes/"]);
        // The main worktree's commands never change the agent's live region (G27, G28).
        let live = agent.region_bytes();
        worktree_dupe(s, &main, &["hide", "scratch/"]);
        commit(s, &main, "Hide scratch for future work");
        assert_eq!(agent.region_bytes(), live);
        assert_eq!(fs::read(agent.root.join(".gitdupe")).unwrap(), b"notes\n");
        write(
            &agent.root,
            "scratch/task.md",
            b"agent scratch adopted by add\n",
        );
        worktree_public_clean(s, &agent);
        let index = agent
            .private(s)
            .git(["ls-files", "-s", "-z"])
            .succeeds()
            .stdout;
        let region = agent.region_bytes();
        for words in [&["status", "--porcelain"][..], &["ls-files", "-s", "-z"]] {
            let warned = worktree_dupe(s, &agent, words);
            assert_eq!(warned.lines("warning"), [SCRATCH_WARNING], "{warned:?}");
            assert_eq!(
                agent
                    .private(s)
                    .git(["ls-files", "-s", "-z"])
                    .succeeds()
                    .stdout,
                index
            );
            assert_eq!(agent.region_bytes(), region);
            assert_eq!(fs::read(agent.root.join(".gitdupe")).unwrap(), b"notes\n");
        }
        let adopted = worktree_dupe(s, &agent, &["add", "scratch/"]);
        assert!(adopted.lines("warning").is_empty(), "{adopted:?}");
        assert_eq!(
            agent
                .private(s)
                .git(["cat-file", "blob", ":scratch/task.md"])
                .succeeds()
                .stdout,
            b"agent scratch adopted by add\n"
        );
        agent
            .private(s)
            .git(["ls-files", "--error-unmatch", "scratch/task.md"])
            .succeeds();
        let quiet = worktree_dupe(s, &agent, &["status", "--porcelain"]);
        assert!(quiet.lines("warning").is_empty(), "{quiet:?}");
        assert_eq!(
            agent.region().unwrap().rules,
            [
                b"/.gitdupe".to_vec(),
                b"/notes".to_vec(),
                b"/scratch".to_vec()
            ]
        );
        commit(s, &agent, "Agent scratch");
        let commit = worktree_private_head(s, &agent);
        let public = || {
            Tree::of(&main.common_directory).without(&[
                &main.private_directory(),
                &agent.private_directory(),
                &exclude,
            ])
        };
        let before = public();
        let live = agent.region_bytes();
        s.git(["dupe", "push", "origin", "HEAD:agent"])
            .from(&agent.root)
            .succeeds();
        assert!(before.changed_in(&public()).is_empty());
        worktree_dupe(s, &main, &["merge", "agent"]);
        assert!(before.changed_in(&public()).is_empty());
        assert_eq!(agent.region_bytes(), live);
        assert_eq!(
            main.private(s)
                .git(["rev-parse", "agent"])
                .succeeds()
                .stdout,
            commit
        );
        main.private(s)
            .git([
                "merge-base",
                "--is-ancestor",
                std::str::from_utf8(commit.trim_ascii_end()).unwrap(),
                "HEAD",
            ])
            .succeeds();
        assert_eq!(
            fs::read(root.join("scratch/task.md")).unwrap(),
            b"agent scratch adopted by add\n"
        );
        assert_eq!(
            fs::read(root.join(".gitdupe")).unwrap(),
            b"notes\nscratch\n"
        );
        assert_eq!(
            main.region().unwrap().rules,
            [
                b"/.gitdupe".to_vec(),
                b"/notes".to_vec(),
                b"/scratch".to_vec()
            ]
        );

        let main_region = main.region_bytes();
        let agent_region = agent.region_bytes();
        fs::write(
            &exclude,
            [TOP, &main_region, BETWEEN, &agent_region, LAST].concat(),
        )
        .unwrap();
        worktree_public_clean(s, &agent);
        for path in [".gitdupe", "notes/plan.md", "notes/x.md", "scratch/task.md"] {
            assert!(agent.root.join(path).is_file(), "{path}");
        }
        s.git(["worktree", "remove", "../project-agent"])
            .from(&root)
            .succeeds();
        assert!(!agent.git_directory.exists());
        assert!(!agent.root.exists());
        assert_eq!(agent.region_bytes(), agent_region);
        worktree_dupe(s, &main, &["status"]);
        assert!(agent.region().is_none());
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(
            fs::read(&exclude).unwrap(),
            [TOP, &main_region, BETWEEN, LAST].concat()
        );
        worktree_public_clean(s, &main);
    });
}
