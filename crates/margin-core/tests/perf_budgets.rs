//! Enforced parse budgets (issue #99).
//!
//! These are hang detectors, not benchmarks: each workload must complete
//! with a sane result well inside a generous wall-clock budget. The budget
//! (60 s) is ~700x the slowest observed debug time, so it can only fail
//! on genuine pathology (a hang or a superlinear blowup), never on a busy
//! shared runner. Actual measurement lives in the criterion benches
//! (`benches/parse.rs`, informational) — see the performance strategy in
//! `docs/architecture.md` for the distinction.
//!
//! Typical times on an M-series Mac: 10k lines ~0.4 ms release / ~8 ms
//! debug; 250k lines ~9 ms release / ~90 ms debug.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use margin_core::parse_unified;

/// Hang-detector budget shared by every workload below.
const BUDGET: Duration = Duration::from_secs(60);

/// 100 files x 100 changed lines: the everyday agent-authored review.
fn medium_patch() -> Vec<u8> {
    let mut out = String::new();
    for f in 0..100 {
        out.push_str(&format!(
            "diff --git a/src/file{f}.rs b/src/file{f}.rs\n\
             index 1111111..2222222 100644\n\
             --- a/src/file{f}.rs\n\
             +++ b/src/file{f}.rs\n\
             @@ -0,0 +1,100 @@\n"
        ));
        for l in 0..100 {
            out.push_str(&format!("+let value_{l} = compute({l}) + {f};\n"));
        }
    }
    out.into_bytes()
}

/// One 250k-line file: the lockfile monster.
fn giant_patch() -> Vec<u8> {
    let mut out =
        String::from("diff --git a/Cargo.lock b/Cargo.lock\nindex 1111111..2222222 100644\n--- a/Cargo.lock\n+++ b/Cargo.lock\n@@ -0,0 +1,250000 @@\n");
    for l in 0..250_000 {
        out.push_str(&format!("+let value_{l} = compute({l});\n"));
    }
    out.into_bytes()
}

#[test]
fn parse_10k_lines_to_completion() {
    let started = Instant::now();
    let outcome = parse_unified(&medium_patch());
    assert!(
        started.elapsed() < BUDGET,
        "10k-line parse took {:?}",
        started.elapsed()
    );
    assert!(outcome.warnings.is_empty());
    assert_eq!(outcome.changeset.files.len(), 100);
    assert_eq!(outcome.changeset.additions(), 10_000);
}

#[test]
fn parse_250k_lines_to_completion() {
    let started = Instant::now();
    let outcome = parse_unified(&giant_patch());
    assert!(
        started.elapsed() < BUDGET,
        "250k-line parse took {:?}",
        started.elapsed()
    );
    assert!(outcome.warnings.is_empty());
    assert_eq!(outcome.changeset.files.len(), 1);
    assert_eq!(outcome.changeset.additions(), 250_000);
}
