//! Word-level intra-line diffing: which parts of a changed line actually
//! changed. Pure functions over strings (ADR-0003); the TUI decides how to
//! render the ranges.

use std::ops::Range;

use similar::utils::diff_words;
use similar::{Algorithm, ChangeTag};

use crate::model::{Hunk, LineKind};

/// Lines longer than this skip word diffing — minified bundles and lockfile
/// blobs produce noise, not signal, and cost real time.
const MAX_LINE_LEN: usize = 1024;

/// Pair deletions with additions the way reviewers read them: the k-th
/// deletion in a run pairs with the k-th addition in the run that follows
/// (the same alignment the side-by-side layout uses).
/// Returns `(deletion_index, addition_index)` pairs into `hunk.lines`.
pub fn paired_changes(hunk: &Hunk) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut dels: Vec<usize> = Vec::new();
    let mut adds: Vec<usize> = Vec::new();

    let flush = |dels: &mut Vec<usize>, adds: &mut Vec<usize>, out: &mut Vec<(usize, usize)>| {
        out.extend(dels.iter().copied().zip(adds.iter().copied()));
        dels.clear();
        adds.clear();
    };

    for (idx, line) in hunk.lines.iter().enumerate() {
        match line.kind {
            LineKind::Deletion => dels.push(idx),
            LineKind::Addition => adds.push(idx),
            LineKind::Context => flush(&mut dels, &mut adds, &mut pairs),
        }
    }
    flush(&mut dels, &mut adds, &mut pairs);
    pairs
}

/// Byte ranges (into `old` / `new`) that differ between a paired deletion
/// and addition. Empty when emphasis would be noise: very long lines, or
/// lines that share less than half their content.
pub fn intraline_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    if old.len() > MAX_LINE_LEN || new.len() > MAX_LINE_LEN {
        return (Vec::new(), Vec::new());
    }

    let changes = diff_words(Algorithm::Myers, old, new);
    let mut old_ranges: Vec<Range<usize>> = Vec::new();
    let mut new_ranges: Vec<Range<usize>> = Vec::new();
    let (mut old_pos, mut new_pos) = (0usize, 0usize);
    let mut equal_bytes = 0usize;

    for (tag, piece) in &changes {
        let len = piece.len();
        match tag {
            ChangeTag::Equal => {
                equal_bytes += len;
                old_pos += len;
                new_pos += len;
            }
            ChangeTag::Delete => {
                push_merged(&mut old_ranges, old_pos..old_pos + len);
                old_pos += len;
            }
            ChangeTag::Insert => {
                push_merged(&mut new_ranges, new_pos..new_pos + len);
                new_pos += len;
            }
        }
    }
    debug_assert_eq!(old_pos, old.len(), "word diff must cover the old line");
    debug_assert_eq!(new_pos, new.len(), "word diff must cover the new line");

    // Mostly-rewritten lines: whole-line coloring reads better than
    // emphasizing nearly everything.
    if equal_bytes * 2 < usize::max(old.len(), new.len()) {
        return (Vec::new(), Vec::new());
    }
    (old_ranges, new_ranges)
}

/// Append a range, merging with the previous one when adjacent.
fn push_merged(ranges: &mut Vec<Range<usize>>, next: Range<usize>) {
    if let Some(last) = ranges.last_mut() {
        if last.end == next.start {
            last.end = next.end;
            return;
        }
    }
    ranges.push(next);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Line;

    fn line(kind: LineKind, content: &str) -> Line {
        Line {
            kind,
            content: content.as_bytes().to_vec(),
            no_newline: false,
        }
    }

    #[test]
    fn pairs_zip_runs_and_reset_on_context() {
        let hunk = Hunk {
            lines: vec![
                line(LineKind::Context, "ctx"),
                line(LineKind::Deletion, "a"),
                line(LineKind::Deletion, "b"),
                line(LineKind::Addition, "A"),
                line(LineKind::Context, "ctx"),
                line(LineKind::Addition, "orphan"),
            ],
            ..Hunk::default()
        };
        // del[1]<->add[3]; del[2] unpaired; add[5] in a fresh run, unpaired.
        assert_eq!(paired_changes(&hunk), vec![(1, 3)]);
    }

    #[test]
    fn ranges_cover_only_the_changed_word() {
        let (old, new) = intraline_ranges("let total = a + b;", "let total = a - b;");
        assert_eq!(old, vec![14..15]);
        assert_eq!(new, vec![14..15]);
    }

    #[test]
    fn ranges_cover_changed_words_and_spare_common_ones() {
        let old_s = "hello old world";
        let new_s = "hello brand new world";
        let (old, new) = intraline_ranges(old_s, new_s);
        let old_text: String = old.iter().map(|r| &old_s[r.clone()]).collect();
        let new_text: String = new.iter().map(|r| &new_s[r.clone()]).collect();
        assert!(old_text.contains("old"), "{old:?} -> {old_text:?}");
        assert!(new_text.contains("brand") && new_text.contains("new"));
        assert!(!old_text.contains("hello") && !old_text.contains("world"));
        assert!(!new_text.contains("hello") && !new_text.contains("world"));
    }

    #[test]
    fn contiguous_deleted_tokens_merge_into_one_range() {
        // "one two" disappears as four adjacent tokens; push_merged must
        // collapse them into a single range. (The line is mostly unchanged,
        // so the rewrite heuristic stays out of the way.)
        let old_s = "keep keep keep one two keep";
        let (old, new) = intraline_ranges(old_s, "keep keep keep keep");
        assert_eq!(old.len(), 1, "adjacent deletions merge: {old:?}");
        let covered = &old_s[old[0].clone()];
        assert!(
            covered.contains("one") && covered.contains("two"),
            "{covered:?}"
        );
        assert!(new.is_empty());
    }

    #[test]
    fn rewritten_lines_get_no_emphasis() {
        let (old, new) = intraline_ranges("completely different", "nothing in common xyz");
        assert!(old.is_empty() && new.is_empty());
    }

    #[test]
    fn overlong_lines_are_skipped() {
        let long = "x".repeat(2000);
        let (old, new) = intraline_ranges(&long, "short");
        assert!(old.is_empty() && new.is_empty());
    }

    #[test]
    fn unicode_lines_do_not_panic_and_align() {
        let (old, new) = intraline_ranges("caf\u{e9} au lait", "caf\u{e9} con leche");
        for r in &old {
            assert!(
                "caf\u{e9} au lait".get(r.clone()).is_some(),
                "old range on char boundary"
            );
        }
        for r in &new {
            assert!("caf\u{e9} con leche".get(r.clone()).is_some());
        }
    }
}
