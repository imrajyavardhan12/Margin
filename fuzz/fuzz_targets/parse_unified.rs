//! End-to-end pipeline fuzz: parse arbitrary bytes as a unified diff, then
//! exercise every downstream consumer the TUI hits — `display_path`,
//! add/delete counts, lossy content, and intra-line emphasis — asserting
//! each one's safety contract, not just absence of panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use margin_core::{intraline_ranges, paired_changes, parse_unified};

fuzz_target!(|data: &[u8]| {
    let outcome = parse_unified(data);
    for file in &outcome.changeset.files {
        // SECURITY.md: a crafted path must never emit control bytes.
        let shown = file.display_path();
        assert!(
            !shown.chars().any(char::is_control),
            "display_path leaked a control character: {shown:?}"
        );
        let _ = file.additions();
        let _ = file.deletions();
        for hunk in &file.hunks {
            for (a, b) in paired_changes(hunk) {
                let (old, new) = match (hunk.lines.get(a), hunk.lines.get(b)) {
                    (Some(old), Some(new)) => (old, new),
                    _ => panic!("paired_changes returned out-of-bounds pair ({a}, {b})"),
                };
                let old_text = old.content_lossy();
                let new_text = new.content_lossy();
                let (old_ranges, new_ranges) = intraline_ranges(&old_text, &new_text);
                check_ranges(&old_text, &old_ranges);
                check_ranges(&new_text, &new_ranges);
            }
        }
    }
});

/// The TUI slices styled spans with these ranges; any range that escapes the
/// string or splits a UTF-8 char is a render-time panic waiting to happen.
fn check_ranges(text: &str, ranges: &[std::ops::Range<usize>]) {
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
