//! Parser benchmarks (ADR-0010): the blueprint budgets parsing as cheap and
//! linear; these keep that honest as features land.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use criterion::{criterion_group, criterion_main, Criterion};

fn synthetic_patch(files: usize, lines_per_file: usize) -> Vec<u8> {
    let mut out = String::new();
    for f in 0..files {
        out.push_str(&format!(
            "diff --git a/src/file{f}.rs b/src/file{f}.rs\n\
             index 1111111..2222222 100644\n\
             --- a/src/file{f}.rs\n\
             +++ b/src/file{f}.rs\n\
             @@ -0,0 +1,{lines_per_file} @@\n"
        ));
        for l in 0..lines_per_file {
            out.push_str(&format!("+let value_{l} = compute({l}) + {f};\n"));
        }
    }
    out.into_bytes()
}

fn benches(c: &mut Criterion) {
    let medium = synthetic_patch(100, 100); // 100 files, 10k lines
    let giant = synthetic_patch(1, 250_000); // the lockfile monster

    c.bench_function("parse/100_files_10k_lines", |b| {
        b.iter(|| margin_core::parse_unified(&medium))
    });
    c.bench_function("parse/250k_line_file", |b| {
        b.iter(|| margin_core::parse_unified(&giant))
    });
}

criterion_group!(parse, benches);
criterion_main!(parse);
