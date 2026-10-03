//! `init` under a template directory of the developer's that holds a `HEAD`.

use std::ffi::{OsStr, OsString};
use std::fs;

use crate::harness::{End, under_each_release};

/// The template is copied as plain `git init` copies it, and its `HEAD` stands in place of
/// the branch `-b` or the public `HEAD` names.
#[test]
fn a_templates_head_stands_in_place_of_the_initial_branch() {
    under_each_release(|s| {
        let template = s.dir().join("template");
        fs::create_dir(&template).unwrap();
        fs::write(template.join("HEAD"), b"ref: refs/heads/trunk\n").unwrap();
        fs::write(template.join("description"), b"from the template\n").unwrap();
        let mut template_dir = OsString::from("init.templateDir=");
        template_dir.push(&template);
        for (index, (options, branch)) in [(&[][..], "main"), (&["-b", "feature"][..], "feature")]
            .into_iter()
            .enumerate()
        {
            let plain = s.dir().join(format!("plain-{index}"));
            let mut words = vec![OsStr::new("-c"), &template_dir, OsStr::new("init")];
            words.extend(options.iter().map(OsStr::new));
            words.extend([OsStr::new("-q"), plain.as_os_str()]);
            s.git(words).succeeds();

            let dir = s.dir().join(format!("workspace-{index}"));
            s.repository(&dir);
            let mut words = vec![OsStr::new("-c"), &template_dir, OsStr::new("dupe")];
            words.push(OsStr::new("init"));
            words.extend(options.iter().map(OsStr::new));
            let output = s.git(words).from(&dir).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.lines("fatal").is_empty(), "{output:?}");

            let head = fs::read(dir.join(".git/dupe/HEAD")).unwrap();
            assert_eq!(head, fs::read(plain.join(".git/HEAD")).unwrap());
            assert_ne!(head, format!("ref: refs/heads/{branch}\n").as_bytes());
            assert_eq!(
                fs::read(dir.join(".git/dupe/description")).unwrap(),
                fs::read(plain.join(".git/description")).unwrap()
            );
            let private = s
                .git(["dupe", "symbolic-ref", "HEAD"])
                .from(&dir)
                .succeeds();
            let public = s.git(["symbolic-ref", "HEAD"]).from(&plain).succeeds();
            assert_eq!(private.stdout, public.stdout);
            let worktree = s
                .private(&dir)
                .git(["config", "--local", "--get", "core.worktree"])
                .succeeds();
            assert_eq!(worktree.stdout, b"../..\n");
        }
    });
}
