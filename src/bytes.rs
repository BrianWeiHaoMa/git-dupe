//! The byte form of a word or a path, and the one place bytes become a name.
//!
//! Below every part: the guards and the runner, which import no part, both need it. Inside
//! the executable a word or a path is bytes, in the form `OsStr::as_encoded_bytes` gives:
//! the bytes themselves on Linux and macOS, on Windows WTF-8, which is UTF-8 for every
//! valid name (`Foundations/Platform`, S20). Every byte string is a name on Linux and macOS;
//! on Windows only valid UTF-8 is, and what is not stands nowhere by `lstat` and cannot
//! stand on a command line (G30).
//!
//! `name` is R11's first site: `owned` and `under` are written over it, and nothing else
//! turns bytes into a name.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The bytes of a name.
pub fn of(name: &OsStr) -> &[u8] {
    name.as_encoded_bytes()
}

/// R11's first site, on Linux and macOS: every byte string is a name.
#[cfg(unix)]
pub fn name(bytes: &[u8]) -> Option<&OsStr> {
    // SAFETY: an `OsStr` on these platforms is any byte string, and `bytes` lives as long
    // as the name made of it.
    Some(unsafe { OsStr::from_encoded_bytes_unchecked(bytes) })
}

/// R11's first site, on Windows: a name is valid UTF-8, and any other bytes are none (G30).
#[cfg(windows)]
pub fn name(bytes: &[u8]) -> Option<&OsStr> {
    std::str::from_utf8(bytes).ok().map(OsStr::new)
}

/// `name` of bytes the caller owns.
pub fn owned(bytes: Vec<u8>) -> Option<OsString> {
    name(&bytes).map(OsStr::to_owned)
}

/// What stands at the root-relative `relative`, as a path below `root`; none for bytes that
/// are no name, at which `lstat` finds nothing (`Holds/G30`).
pub fn under(root: &Path, relative: &[u8]) -> Option<PathBuf> {
    name(relative).map(|name| root.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::names_every_byte_string;

    #[test]
    fn valid_utf8_is_a_name_everywhere_and_comes_back_as_it_went_in() {
        let name = name(b"caf\xc3\xa9").expect("a name");
        assert_eq!(of(name), b"caf\xc3\xa9");
        assert_eq!(owned(b"caf\xc3\xa9".to_vec()).as_deref(), Some(name));
    }

    #[test]
    fn other_bytes_are_a_name_only_where_every_byte_string_is_one() {
        for bytes in [&b"caf\xe9"[..], b"\xff", b"a\xffb"] {
            assert_eq!(name(bytes).is_some(), names_every_byte_string());
            assert_eq!(owned(bytes.to_vec()).is_some(), names_every_byte_string());
            if let Some(name) = name(bytes) {
                assert_eq!(of(name), bytes);
            }
        }
    }

    #[test]
    fn under_joins_a_name_and_finds_no_path_for_bytes_that_are_none() {
        let root = std::env::temp_dir();
        assert_eq!(under(&root, b"notes"), Some(root.join("notes")));
        assert_eq!(under(&root, b"\xff").is_some(), names_every_byte_string());
    }
}
