//! The release archive: this package built in the release profile, and the executable,
//! the manual page that build rendered, and the install note `INSTALL` from the
//! repository root packed into `git-dupe-<version>.tar` beside the executable, all three
//! under the one directory `git-dupe-<version>`. It is made anew by every check command
//! that needs it and replaces whole the one an earlier command made, which holds the
//! binary of an earlier source; a build that fails ends the command's archive checks
//! instead. Needs `tar`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use super::fresh::FreshDirectory;

/// What one check command made.
pub struct ReleaseArchive {
    /// `git-dupe-<version>.tar`, beside the release executable.
    pub path: PathBuf,
    /// The release executable, where Cargo named it for this build.
    pub executable: PathBuf,
    /// The page that build rendered, in the output directory Cargo named for its build
    /// script.
    pub page: PathBuf,
}

/// The archive of this check command, made by its first caller; the others wait for it.
pub fn release_archive() -> &'static ReleaseArchive {
    static MADE: OnceLock<Result<ReleaseArchive, String>> = OnceLock::new();
    match MADE.get_or_init(make) {
        Ok(made) => made,
        Err(cause) => panic!("{cause}"),
    }
}

/// The install note, packed as the repository holds it.
pub fn install_note() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("INSTALL")
}

/// The command lines of an install note: each line marked `$ `, taken as written.
pub fn note_commands(note: &str) -> Vec<String> {
    note.lines()
        .filter_map(|line| line.trim_start().strip_prefix("$ "))
        .map(str::to_owned)
        .collect()
}

fn make() -> Result<ReleaseArchive, String> {
    let (executable, out_dir) = release_build()?;
    let page = out_dir.join("git-dupe.1");
    let leading = format!("git-dupe-{}", env!("CARGO_PKG_VERSION"));

    // Staged with the modes an archive for anyone needs, whatever the builder's umask.
    let staging = FreshDirectory::create();
    let staged = staging.path().join(&leading);
    fs::create_dir(&staged).map_err(because("create", &staged))?;
    let note = install_note();
    let members = [
        (&executable, "git-dupe", 0o755),
        (&page, "git-dupe.1", 0o644),
        (&note, "INSTALL", 0o644),
    ];
    for (from, name, mode) in members {
        let to = staged.join(name);
        fs::copy(from, &to).map_err(because("copy", from))?;
        fs::set_permissions(&to, fs::Permissions::from_mode(mode))
            .map_err(because("set the mode of", &to))?;
    }

    // Packed beside the archive and renamed over it, so that the path holds a whole
    // archive or none. The files are named one by one: no entry for their directory.
    let path = executable.with_file_name(format!("{leading}.tar"));
    let packing = executable.with_file_name(format!("{leading}.tar.{}", std::process::id()));
    let mut tar = Command::new("tar");
    tar.arg("--create")
        .arg("--format=ustar")
        .arg("--file")
        .arg(&packing)
        .arg("--directory")
        .arg(staging.path())
        .args(members.map(|(_, name, _)| format!("{leading}/{name}")));
    let packed = run(&mut tar).and_then(|_| {
        fs::rename(&packing, &path).map_err(because("rename onto the archive", &packing))
    });
    if packed.is_err() {
        let _ = fs::remove_file(&packing);
    }
    packed?;
    Ok(ReleaseArchive {
        path,
        executable,
        page,
    })
}

/// Builds the package in the release profile and returns the executable and the build
/// script's output directory as Cargo names them in its messages.
fn release_build() -> Result<(PathBuf, PathBuf), String> {
    let said = run(Command::new(env!("CARGO"))
        .args([
            "build",
            "--release",
            "--locked",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR")))?;
    named(&said)
}

/// The executable and the output directory that Cargo's messages name, one each: a
/// `compiler-artifact` line with an `executable`, the build script's having none, and a
/// `build-script-executed` line with its `out_dir`. One package is built.
fn named(said: &str) -> Result<(PathBuf, PathBuf), String> {
    let field = |reason: &str, name: &str| -> Result<Vec<PathBuf>, String> {
        let opening = format!("{{\"reason\":\"{reason}\",");
        said.lines()
            .filter(|line| line.starts_with(&opening))
            .filter_map(|line| string_field(line, name).transpose())
            .collect()
    };
    let executables = field("compiler-artifact", "executable")?;
    let out_dirs = field("build-script-executed", "out_dir")?;
    match (&executables[..], &out_dirs[..]) {
        ([executable], [out_dir]) if executable.ends_with("git-dupe") => {
            Ok((executable.clone(), out_dir.clone()))
        }
        _ => Err(format!(
            "the release build named the executables {executables:?} and the output \
             directories {out_dirs:?}, where one `git-dupe` and one directory were expected:\n\
             {said}"
        )),
    }
}

/// The value of the string field `name` of a one-line JSON message, or none where the
/// line holds no such string. A value Cargo escaped is not read here.
fn string_field(line: &str, name: &str) -> Result<Option<PathBuf>, String> {
    let opening = format!("\"{name}\":\"");
    let Some((_, rest)) = line.split_once(&opening) else {
        return Ok(None);
    };
    match rest.split_once('"') {
        Some((value, _)) if !value.contains('\\') => Ok(Some(PathBuf::from(value))),
        _ => Err(format!("cannot read the field {name} of: {line}")),
    }
}

/// Runs a tool to its end and returns what it printed; a failure carries the command and
/// the end of what it said on standard error.
fn run(command: &mut Command) -> Result<String, String> {
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|cause| format!("cannot run {command:?}: {cause}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = said.lines().collect();
        let tail = lines[lines.len().saturating_sub(40)..].join("\n");
        return Err(format!("{command:?}: {}\n{tail}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|cause| format!("{command:?}: {cause}"))
}

fn because(act: &str, path: &Path) -> impl Fn(std::io::Error) -> String {
    move |cause| format!("cannot {act} {}: {cause}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD_SCRIPT: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///p#0.1.0","target":{"kind":["custom-build"],"name":"build-script-build"},"executable":null,"fresh":true}"#;
    const EXECUTED: &str = r#"{"reason":"build-script-executed","package_id":"path+file:///p#0.1.0","linked_libs":[],"env":[],"out_dir":"/p/target/release/build/git-dupe-1/out"}"#;
    const BINARY: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///p#0.1.0","target":{"kind":["bin"],"name":"git-dupe"},"executable":"/p/target/release/git-dupe","fresh":false}"#;
    const FINISHED: &str = r#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn the_executable_and_the_output_directory_are_the_ones_cargo_named() {
        let said = [BUILD_SCRIPT, EXECUTED, BINARY, FINISHED].join("\n");
        assert_eq!(
            named(&said).unwrap(),
            (
                PathBuf::from("/p/target/release/git-dupe"),
                PathBuf::from("/p/target/release/build/git-dupe-1/out")
            )
        );
    }

    #[test]
    fn a_build_that_names_neither_or_two_is_not_read() {
        for said in [
            [BUILD_SCRIPT, FINISHED].join("\n"),
            [BUILD_SCRIPT, EXECUTED, FINISHED].join("\n"),
            [EXECUTED, BINARY, BINARY, FINISHED].join("\n"),
            [EXECUTED, EXECUTED, BINARY].join("\n"),
            [
                EXECUTED,
                &BINARY.replace("release/git-dupe\"", "release/other\""),
            ]
            .join("\n"),
        ] {
            assert!(named(&said).is_err(), "{said}");
        }
        let escaped = BINARY.replace("/p/target", "/p\\\"q/target");
        assert!(named(&[EXECUTED, &escaped].join("\n")).is_err());
    }
}
