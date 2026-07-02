//! Fuzz the ANSI stripper that guards the `margin pager` parse path.
//! Contracts: never grows the input, never lets an ESC byte through, and is
//! idempotent. The stripped bytes then feed the parser, mirroring how
//! colorized git output actually flows in.
#![no_main]

use libfuzzer_sys::fuzz_target;
use margin_core::{parse_unified, strip_ansi};

fuzz_target!(|data: &[u8]| {
    let stripped = strip_ansi(data);
    assert!(stripped.len() <= data.len(), "strip_ansi grew the input");
    assert!(!stripped.contains(&0x1b), "ESC byte survived strip_ansi");
    assert_eq!(
        strip_ansi(&stripped),
        stripped,
        "strip_ansi is not idempotent"
    );
    let _ = parse_unified(&stripped);
});
