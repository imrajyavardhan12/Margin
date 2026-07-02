//! Fuzz intra-line emphasis on arbitrary string pairs, unconstrained by
//! diff-line shape. Every returned range must be sliceable from its source
//! string: in bounds, ordered, and on UTF-8 char boundaries.
#![no_main]

use libfuzzer_sys::fuzz_target;
use margin_core::intraline_ranges;

fuzz_target!(|input: (&str, &str)| {
    let (old, new) = input;
    let (old_ranges, new_ranges) = intraline_ranges(old, new);
    for (text, ranges) in [(old, &old_ranges), (new, &new_ranges)] {
        for r in ranges {
            assert!(
                r.start <= r.end && r.end <= text.len(),
                "range {r:?} out of bounds for len {}",
                text.len()
            );
            assert!(
                text.is_char_boundary(r.start) && text.is_char_boundary(r.end),
                "range {r:?} splits a UTF-8 character"
            );
        }
    }
});
