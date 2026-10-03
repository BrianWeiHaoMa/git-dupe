//! Transfer guard and settle run counts do not follow remote or file counts (G22, R4).

use crate::harness::{End, daily_state, private_commit, run_traced, under_each_release, write};

#[test]
fn transfer_runs_do_not_follow_remote_or_private_file_counts() {
    under_each_release(|s| {
        let commands = [&["push", "origin", "main"][..], &["fetch"], &["pull"]];
        let mut sequences = Vec::new();
        for remotes in [1, 20] {
            for files in [5, 500] {
                let root = s.dir().join(format!("project-{remotes}-{files}"));
                daily_state(s, &root);
                for index in 5..files {
                    write(&root, &format!("notes/file-{index}"), b"private\n");
                }
                if files > 5 {
                    s.private(&root).git(["add", "notes"]).succeeds();
                    private_commit(s, &root);
                }
                let listed = s.private(&root).git(["ls-files", "-z"]).succeeds();
                assert_eq!(
                    listed.stdout.iter().filter(|&&byte| byte == 0).count(),
                    files
                );
                let public = s.dir().join(format!("public-{remotes}-{files}.git"));
                let private = s.dir().join(format!("private-{remotes}-{files}.git"));
                s.bare_repository(&public);
                s.bare_repository(&private);
                for index in 0..remotes {
                    let name = if index == 0 {
                        "origin".to_owned()
                    } else {
                        format!("r{index}")
                    };
                    s.git(["remote", "add", &name, public.to_str().unwrap()])
                        .from(&root)
                        .succeeds();
                    s.git(["dupe", "remote", "add", &name, private.to_str().unwrap()])
                        .from(&root)
                        .succeeds();
                }
                s.git(["dupe", "push", "-u", "origin", "main"])
                    .from(&root)
                    .succeeds();
                let mut fixture = Vec::new();
                for words in commands {
                    let (output, runs) = run_traced(
                        s.git(["dupe"].into_iter().chain(words.iter().copied()))
                            .from(&root),
                        &s.dir().join("trace"),
                    );
                    assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
                    let own = runs.own();
                    let tail: Vec<Vec<u8>> = ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied())
                        .map(|word| word.as_bytes().to_vec())
                        .collect();
                    assert_eq!(
                        own.words()
                            .iter()
                            .filter(|run| run.ends_with(&tail))
                            .count(),
                        1,
                        "{own:?}"
                    );
                    let mut expected = vec!["rev-parse", "config", "worktree", "config"];
                    if words.len() == 1 {
                        expected.push("symbolic-ref");
                    }
                    expected.extend([words[0], "ls-files", "ls-files", "check-ignore"]);
                    let sequence: Vec<Vec<u8>> =
                        own.commands().into_iter().map(<[u8]>::to_vec).collect();
                    assert_eq!(
                        sequence,
                        expected
                            .iter()
                            .map(|word| word.as_bytes().to_vec())
                            .collect::<Vec<_>>(),
                        "{own:?}"
                    );
                    fixture.push(sequence);
                }
                sequences.push(fixture);
            }
        }
        for sequence in &sequences[1..] {
            assert_eq!(sequence, &sequences[0]);
        }
    });
}
