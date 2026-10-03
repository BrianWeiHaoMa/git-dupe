//! An alias's leading global options as the prefix of the runs of the command it reaches:
//! a `--config-env` among them is read in each Git process git-dupe runs, where the
//! variables a private run sets or removes hold that run's values (G19, G10).

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{End, daily_state, under_each_release};

#[test]
fn a_config_env_in_an_alias_reads_the_private_runs_own_variables() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        let public = dir.join(".git");
        for (name, variable) in [("where", "GIT_DIR"), ("index", "GIT_INDEX_FILE")] {
            let expansion = format!("--config-env=remote.origin.url={variable} remote -v");
            s.private(&dir)
                .git(["config", &format!("alias.{name}"), &expansion])
                .succeeds();
        }

        // A caller whose GIT_DIR names the public repository, as `--git-dir` before
        // `dupe` or a public hook gives it: the private run sets its own, and the alias
        // reads that one.
        let ours = s
            .git([
                OsStr::new("--git-dir"),
                public.as_os_str(),
                OsStr::new("--work-tree"),
                dir.as_os_str(),
                OsStr::new("dupe"),
                OsStr::new("where"),
            ])
            .from(&dir)
            .run();
        assert_eq!(ours.end, End::Code(0), "{ours:?}");
        assert!(ours.stderr.is_empty(), "{ours:?}");
        // Git's own `remote -v` for a URL standing in for the value read gives the form;
        // the value in its place must be the private Git directory, however spelled.
        let stand_in = "/stand-in";
        let form = s
            .private(&dir)
            .git([
                "-c",
                &format!("remote.origin.url={stand_in}"),
                "remote",
                "-v",
            ])
            .succeeds()
            .stdout;
        let parts = split_around(&form, stand_in.as_bytes());
        assert_eq!(parts.len(), 3, "{}", form.escape_ascii());
        let value = ours.stdout[parts[0].len()..]
            .windows(parts[1].len())
            .position(|window| window == parts[1])
            .map(|end| &ours.stdout[parts[0].len()..parts[0].len() + end])
            .unwrap_or_else(|| panic!("{ours:?}"));
        let expected = [parts[0], value, parts[1], value, parts[2]].concat();
        assert_eq!(ours.stdout, expected, "{ours:?}");
        let read = Path::new(OsStr::from_bytes(value));
        assert_eq!(
            fs::canonicalize(read).unwrap(),
            fs::canonicalize(dir.join(".git/dupe")).unwrap(),
            "{ours:?}"
        );

        // A caller's GIT_INDEX_FILE, which no private run receives: the alias's run has
        // none, and ends as Git ends that expansion without it.
        let gits = s
            .private(&dir)
            .git([
                "--config-env=remote.origin.url=GIT_INDEX_FILE",
                "remote",
                "-v",
            ])
            .run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        let ours = s
            .git(["dupe", "index"])
            .from(&dir)
            .variable("GIT_INDEX_FILE", public.join("index"))
            .run();
        assert_eq!(ours.end, gits.end, "{ours:?}; Git: {gits:?}");
        assert!(ours.stdout.is_empty(), "{ours:?}");
        assert!(
            ours.stderr.starts_with(&gits.stderr),
            "{ours:?}; Git: {gits:?}"
        );
    });
}

/// `text` cut at each occurrence of `part`.
fn split_around<'t>(text: &'t [u8], part: &[u8]) -> Vec<&'t [u8]> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.windows(part.len()).position(|window| window == part) {
        pieces.push(&rest[..at]);
        rest = &rest[at + part.len()..];
    }
    pieces.push(rest);
    pieces
}
