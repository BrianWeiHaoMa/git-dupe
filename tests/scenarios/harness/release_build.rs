//! This package built for the release's target: the target the platform module names where
//! the release's is not the host's, on Linux the static `x86_64-unknown-linux-musl`, else
//! the host's (P3). Its release-profile build is what the archive packs
//! (`release_archive.rs`); its build in the profile `cargo test` builds is the git-dupe every
//! scenario runs, so that on Linux the scenarios run the build the archive packs. Each is
//! read from Cargo's messages, never from a path guessed at.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use super::platform::release_target;

/// The git-dupe every scenario runs (`Composition/The checks`): where the release has a
/// target of its own, on Linux, this package built for that target in the profile
/// `cargo test` builds, once per check command and by its first caller, so that the
/// scenarios run the build the archive packs whatever target `cargo test` was given (P3);
/// elsewhere the executable Cargo built for these checks.
pub fn tested_executable() -> &'static Path {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let built = BUILT.get_or_init(|| match release_target() {
        Some(_) => built_for_the_release(false).map(|(executable, _)| executable),
        None => Ok(PathBuf::from(env!("CARGO_BIN_EXE_git-dupe"))),
    });
    match built {
        Ok(executable) => executable,
        Err(cause) => panic!("{cause}"),
    }
}

/// Builds the package, in the release profile or the one `cargo test` builds, for the
/// release's target where it is not the host's, and returns the executable and the build
/// script's output directory as Cargo names them in its messages. The target is the
/// platform module's alone, never the one these checks were built for (P3).
/// `.cargo/config.toml` reaches this build as it reaches every other (S25).
pub fn built_for_the_release(release: bool) -> Result<(PathBuf, PathBuf), String> {
    let mut cargo = Command::new(env!("CARGO"));
    cargo.args([
        "build",
        "--locked",
        "--message-format=json-render-diagnostics",
    ]);
    if release {
        cargo.arg("--release");
    }
    let target = release_target();
    if let Some(target) = target {
        cargo.args(["--target", target]);
    }
    let said =
        run(cargo.current_dir(env!("CARGO_MANIFEST_DIR"))).map_err(|cause| match target {
            Some(target) => format!(
                "{cause}
the release's executable is built for {target} here, which `rustup target add {target}` installs"
            ),
            None => cause,
        })?;
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
        ([executable], [out_dir]) if names_the_executable(executable) => {
            Ok((executable.clone(), out_dir.clone()))
        }
        _ => Err(format!(
            "the release build named the executables {executables:?} and the output \
             directories {out_dirs:?}, where one `git-dupe` and one directory were expected:\n\
             {said}"
        )),
    }
}

/// Whether `path`'s last component, after either separator, is the package's executable:
/// read from the text, because the build machine's `Path` is not the message's.
fn names_the_executable(path: &Path) -> bool {
    let path = path.to_string_lossy();
    let name = path.rsplit(['/', '\\']).next();
    matches!(name, Some("git-dupe" | "git-dupe.exe"))
}

/// The value of the string field `name` of a one-line JSON message, or none where the
/// line holds no such string. Of Cargo's escapes only `\\` is read, the separator of a
/// Windows path.
fn string_field(line: &str, name: &str) -> Result<Option<PathBuf>, String> {
    let opening = format!("\"{name}\":\"");
    let Some((_, rest)) = line.split_once(&opening) else {
        return Ok(None);
    };
    let mut value = String::new();
    let mut characters = rest.chars();
    loop {
        match characters.next() {
            Some('"') => return Ok(Some(PathBuf::from(value))),
            Some('\\') if characters.next() == Some('\\') => value.push('\\'),
            Some('\\') | None => return Err(format!("cannot read the field {name} of: {line}")),
            Some(character) => value.push(character),
        }
    }
}

/// Runs a tool to its end and returns what it printed; a failure carries the command and
/// the end of what it said on standard error.
pub fn run(command: &mut Command) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows's path as Cargo escapes it, on every platform: `\\` read, every other escape
    /// refused.
    #[test]
    fn a_windows_path_is_read_from_its_escaped_separators() {
        const OUT: &str = r#"{"reason":"build-script-executed","out_dir":"C:\\p\\out"}"#;
        const EXE: &str =
            r#"{"reason":"compiler-artifact","executable":"C:\\p\\release\\git-dupe.exe"}"#;
        assert_eq!(
            named(&[EXE, OUT].join("\n")).unwrap(),
            (
                PathBuf::from(r"C:\p\release\git-dupe.exe"),
                PathBuf::from(r"C:\p\out")
            )
        );
        let quoted = EXE.replace(r"C:\\p", r#"C:\"p"#);
        assert!(named(&[&quoted, OUT].join("\n")).is_err());
        let unknown = EXE.replace(r"C:\\p", r"C:\up");
        assert!(named(&[&unknown, OUT].join("\n")).is_err());
    }

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
