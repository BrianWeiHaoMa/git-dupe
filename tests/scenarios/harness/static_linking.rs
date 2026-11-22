//! What a Linux executable asks of the machine it runs on before its own code runs, read
//! from its ELF headers (P3): the program interpreter its `PT_INTERP` segment names, and the
//! shared libraries its dynamic section's `DT_NEEDED` entries name. A static executable,
//! `static-pie` as Rust links one for `x86_64-unknown-linux-musl`, keeps a dynamic section
//! for its own relocation and names neither. Reads the 64-bit little-endian ELF file an
//! x86-64 Linux build makes, and panics on any other.

const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;

/// What the executable asks of the machine before it runs.
#[derive(Debug, PartialEq, Eq)]
pub struct RunTimeNeeds {
    /// The program interpreter it names, the dynamic loader of a C library.
    pub interpreter: Option<String>,
    /// How many shared libraries it names.
    pub shared_libraries: usize,
}

/// The needs of the executable `bytes`, which a failure calls `what`.
pub fn run_time_needs(what: &str, bytes: &[u8]) -> RunTimeNeeds {
    let at = |start: usize, size: usize| -> u64 {
        let field = bytes
            .get(start..start + size)
            .unwrap_or_else(|| panic!("{what} ends before byte {}", start + size));
        field
            .iter()
            .rev()
            .fold(0, |n, &byte| n << 8 | u64::from(byte))
    };
    assert!(
        bytes.starts_with(b"\x7fELF\x02\x01"),
        "{what} is not a 64-bit little-endian ELF file"
    );
    let (table, entry, count) = (at(0x20, 8) as usize, at(0x36, 2) as usize, at(0x38, 2));
    let mut needs = RunTimeNeeds {
        interpreter: None,
        shared_libraries: 0,
    };
    for header in (0..count as usize).map(|n| table + n * entry) {
        let (offset, size) = (at(header + 8, 8) as usize, at(header + 32, 8) as usize);
        match at(header, 4) as u32 {
            PT_INTERP => {
                let named = &bytes[offset..offset + size];
                let named = named.strip_suffix(b"\0").unwrap_or(named);
                needs.interpreter = Some(String::from_utf8_lossy(named).into_owned());
            }
            PT_DYNAMIC => {
                let tags = (offset..offset + size)
                    .step_by(16)
                    .map(|entry| at(entry, 8));
                needs.shared_libraries += tags
                    .take_while(|&tag| tag != DT_NULL)
                    .filter(|&tag| tag == DT_NEEDED)
                    .count();
            }
            _ => {}
        }
    }
    needs
}
