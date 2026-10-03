//! Transfer short help keeps Git's complete output and release-dependent status (G24).

use crate::harness::under_each_release;

/// Attached help is a private run; outside help receives the original environment.
#[test]
fn transfer_help_is_gits_own_inside_and_outside() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        for command in ["push", "pull", "fetch"] {
            let before = t.everything();
            let expected = s
                .private(&t.root)
                .git(["-c", "help.autocorrect=0", command, "-h"])
                .run();
            let ours = s.git(["dupe", command, "-h"]).from(&t.root).run();
            assert_eq!(ours, expected, "{command} attached");
            before.unchanged();
            let expected = s.git([command, "-h"]).run();
            let ours = s.git(["dupe", command, "-h"]).run();
            assert_eq!(ours, expected, "{command} outside");
        }
    });
}

/// A `-h` after a `--` that Git may take as an option's value is still a help request of
/// a transfer command: where no workspace is attached its words run as Git's own run of
/// them there, after the guard, and outside any repository at once.
#[test]
fn a_help_request_after_a_separator_runs_where_no_workspace_is_attached() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let plain = s.dir().join("plain");
        s.repository(&plain);
        let git_directory = t.root.join(".git");
        for command in ["push", "pull", "fetch"] {
            let words = [command, "-o", "--", "-h"];
            let help = ["-c", "help.autocorrect=0"];
            let expected = s.private(&plain).git(help.into_iter().chain(words)).run();
            let ours = s.git(["dupe"].into_iter().chain(words)).from(&plain).run();
            assert_eq!(ours, expected, "{command} not attached");
            let before = t.everything();
            let expected = s
                .git(help.into_iter().chain(words))
                .variable("GIT_DIR", git_directory.join("dupe"))
                .from(&git_directory)
                .run();
            let ours = s
                .git(["dupe"].into_iter().chain(words))
                .from(&git_directory)
                .run();
            assert_eq!(ours, expected, "{command} inside the Git directory");
            before.unchanged();
            let expected = s.git(words).run();
            let ours = s.git(["dupe"].into_iter().chain(words)).run();
            assert_eq!(ours, expected, "{command} outside");
        }
    });
}
