//! The Git runs of `init`, `hide`, and `unhide` follow neither repository's file count
//! (G22, R4): the same sequence of commands with a few files on each side and with a
//! hundred times as many.

use crate::harness::{End, private_add, private_commit, run_traced, under_each_release, write};

#[test]
fn init_hide_and_unhide_run_git_the_same_number_of_times_whatever_either_repository_holds() {
    under_each_release(|s| {
        let mut sequences = Vec::new();
        for files in [5, 500] {
            let dir = s.dir().join(format!("project-{files}"));
            s.unattached_project(&dir);
            for index in 0..files {
                write(&dir, &format!("src/file-{index}"), b"public\n");
            }
            s.git(["add", "src"]).from(&dir).succeeds();
            s.commit_public(&dir);
            let trace = s.dir().join(format!("trace-{files}"));

            let (init, init_runs) = run_traced(s.git(["dupe", "init"]).from(&dir), &trace);
            assert_eq!(init.end, End::Code(0), "{init:?}");

            write(&dir, ".gitdupe", b"notes\n");
            for index in 0..files {
                write(&dir, &format!("notes/file-{index}"), b"private\n");
            }
            private_add(s, &dir, "notes");
            private_add(s, &dir, ".gitdupe");
            private_commit(s, &dir);
            let tracked = s.private(&dir).git(["ls-files", "-z"]).succeeds();
            assert_eq!(
                tracked.stdout.iter().filter(|&&byte| byte == 0).count(),
                files + 1
            );
            s.git(["dupe", "status"]).from(&dir).succeeds();

            let (hide, hide_runs) =
                run_traced(s.git(["dupe", "hide", "scratch"]).from(&dir), &trace);
            assert_eq!(hide.end, End::Code(0), "{hide:?}");
            let (unhide, unhide_runs) =
                run_traced(s.git(["dupe", "unhide", "scratch"]).from(&dir), &trace);
            assert_eq!(unhide.end, End::Code(0), "{unhide:?}");

            let sequence: Vec<Vec<Vec<u8>>> = [init_runs, hide_runs, unhide_runs]
                .iter()
                .map(|runs| {
                    runs.own()
                        .commands()
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect()
                })
                .collect();
            sequences.push(sequence);
        }
        assert_eq!(sequences[0], sequences[1]);
    });
}
