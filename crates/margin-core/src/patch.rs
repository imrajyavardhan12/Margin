//! Tolerant parser for unified diffs, including git's extended headers.
//!
//! ## Design: tolerant, not strict (ADR-0009, ADR-0010)
//!
//! Margin is a *viewer*: its job is to render whatever `git diff`, `git log
//! -p`, `diff -u`, or a mail-mangled patch file contains. The parser
//! therefore never fails outright — it extracts every changeset it can
//! recognize, skips what it cannot (commit headers, mail headers, binary
//! patch data), and reports anomalies as [`ParseWarning`]s with line numbers
//! so the UI can surface them. Malformed input yields an empty or partial
//! [`Changeset`] plus warnings; it must never panic (fuzz-enforced).
//!
//! Recognized per file:
//! - `diff --git a/X b/Y` headers, with C-quoted paths
//! - extended headers: `old/new mode`, `new/deleted file mode`,
//!   `similarity/dissimilarity index`, `rename/copy from/to`, `index`
//! - `Binary files ... differ` and `GIT binary patch` bodies
//! - `---` / `+++` markers (with `/dev/null` and trailing-tab/timestamp forms)
//! - `@@` hunk headers with optional counts and section headings
//! - `\ No newline at end of file` markers
//! - plain (non-git) unified diffs, detected as `---`/`+++`/`@@` triples

use crate::model::{ByteStr, Changeset, FileDiff, FileStatus, Hunk, Line, LineKind};

/// Result of parsing: always a changeset, plus anything suspicious.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseOutcome {
    pub changeset: Changeset,
    pub warnings: Vec<ParseWarning>,
}

/// A non-fatal anomaly, pointing at the 1-based input line that caused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}

/// Parse a unified diff (git or plain) from raw bytes. Never fails, never
/// panics; see module docs for the tolerance contract.
pub fn parse_unified(input: &[u8]) -> ParseOutcome {
    Parser::new(input).run()
}

struct Parser<'a> {
    lines: Vec<&'a [u8]>,
    pos: usize,
    files: Vec<FileDiff>,
    warnings: Vec<ParseWarning>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        let mut lines: Vec<&[u8]> = input.split(|&b| b == b'\n').collect();
        // `"a\n"` is one line, not two: drop the phantom element produced by
        // splitting after a trailing newline. (Interior empty lines remain
        // and are tolerated as empty context.)
        if input.ends_with(b"\n") {
            lines.pop();
        }
        // Transport tolerance: when *every* line ends with `\r`, the whole
        // patch went through CRLF conversion (Windows checkout, mail
        // gateway); strip the artifact to recover the original bytes. Mixed
        // endings mean the `\r`s are genuine file content — git emits LF
        // metadata even when diffing CRLF files — so they are preserved.
        if !lines.is_empty() && lines.iter().all(|l| l.ends_with(b"\r")) {
            for line in &mut lines {
                *line = line.get(..line.len() - 1).unwrap_or(b"");
            }
        }
        Self {
            lines,
            pos: 0,
            files: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn run(mut self) -> ParseOutcome {
        while self.pos < self.lines.len() {
            let line = self.current();
            if line.strip_prefix(b"diff --git ").is_some() {
                self.parse_git_file();
            } else if self.looks_like_plain_diff_start() {
                self.parse_plain_file();
            } else {
                // Junk between diffs: commit/mail headers, log messages, etc.
                self.pos += 1;
            }
        }
        ParseOutcome {
            changeset: Changeset { files: self.files },
            warnings: self.warnings,
        }
    }

    fn current(&self) -> &'a [u8] {
        self.lines.get(self.pos).copied().unwrap_or(b"")
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(ParseWarning {
            line: self.pos + 1,
            message: message.into(),
        });
    }

    /// A plain `diff -u` file starts with `---` + `+++` + `@@` in sequence.
    /// Requiring all three avoids false positives on prose containing `---`.
    fn looks_like_plain_diff_start(&self) -> bool {
        let at = |off: usize| self.lines.get(self.pos + off).copied().unwrap_or(b"");
        at(0).starts_with(b"--- ")
            && at(1).starts_with(b"+++ ")
            && parse_hunk_header(at(2)).is_some()
    }

    fn parse_plain_file(&mut self) {
        let old = marker_path(self.current(), b"--- ", false);
        self.pos += 1;
        let new = marker_path(self.current(), b"+++ ", false);
        self.pos += 1;

        let status = match (&old, &new) {
            (None, Some(_)) => FileStatus::Added,
            (Some(_), None) => FileStatus::Deleted,
            _ => FileStatus::Modified,
        };
        let mut file = FileDiff {
            old_path: old,
            new_path: new,
            status,
            ..FileDiff::default()
        };
        self.parse_hunks(&mut file);
        self.files.push(file);
    }

    fn parse_git_file(&mut self) {
        let header = self.current();
        let rest = header.strip_prefix(b"diff --git ").unwrap_or(b"");
        let (mut old_path, mut new_path) = git_header_paths(rest);
        self.pos += 1;

        let mut file = FileDiff::default();
        let mut explicit_added = false;
        let mut explicit_deleted = false;
        let mut renamed = false;
        let mut copied = false;

        loop {
            let line = self.current();
            if self.pos >= self.lines.len() || line.starts_with(b"diff --git ") {
                break;
            }
            if let Some(v) = line.strip_prefix(b"old mode ") {
                file.old_mode = parse_octal(v);
            } else if let Some(v) = line.strip_prefix(b"new mode ") {
                file.new_mode = parse_octal(v);
            } else if let Some(v) = line.strip_prefix(b"new file mode ") {
                file.new_mode = parse_octal(v);
                explicit_added = true;
            } else if let Some(v) = line.strip_prefix(b"deleted file mode ") {
                file.old_mode = parse_octal(v);
                explicit_deleted = true;
            } else if let Some(v) = line.strip_prefix(b"similarity index ") {
                file.similarity = parse_percent(v);
            } else if line.strip_prefix(b"dissimilarity index ").is_some() {
                // Rewrite diffs (-B); nothing to record beyond the hunks.
            } else if let Some(v) = line.strip_prefix(b"rename from ") {
                old_path = Some(unquote_value(v));
                renamed = true;
            } else if let Some(v) = line.strip_prefix(b"rename to ") {
                new_path = Some(unquote_value(v));
                renamed = true;
            } else if let Some(v) = line.strip_prefix(b"copy from ") {
                old_path = Some(unquote_value(v));
                copied = true;
            } else if let Some(v) = line.strip_prefix(b"copy to ") {
                new_path = Some(unquote_value(v));
                copied = true;
            } else if line.strip_prefix(b"index ").is_some() {
                // Blob hashes; not needed for rendering.
            } else if line.starts_with(b"Binary files ") || line == b"GIT binary patch" {
                file.is_binary = true;
                self.pos += 1;
                self.skip_binary_body();
                break;
            } else if line.starts_with(b"--- ") {
                if let Some(p) = marker_path(line, b"--- ", true) {
                    old_path = Some(p);
                } else {
                    old_path = None; // /dev/null
                }
                self.pos += 1;
                let plus = self.current();
                if plus.starts_with(b"+++ ") {
                    match marker_path(plus, b"+++ ", true) {
                        Some(p) => new_path = Some(p),
                        None => new_path = None, // /dev/null
                    }
                    self.pos += 1;
                } else {
                    self.warn("`---` marker not followed by `+++`");
                }
                self.parse_hunks(&mut file);
                break;
            } else if parse_hunk_header(line).is_some() {
                // Defensive: hunks without ---/+++ markers.
                self.parse_hunks(&mut file);
                break;
            } else {
                // Anything else ends this file's header section (e.g. the
                // next commit header in `git log -p` after a no-hunk diff).
                break;
            }
            self.pos += 1;
        }

        // /dev/null markers above set sides to None; an explicit
        // new/deleted-file header is the stronger signal.
        if explicit_added {
            old_path = None;
        }
        if explicit_deleted {
            new_path = None;
        }
        file.status = if renamed {
            FileStatus::Renamed
        } else if copied {
            FileStatus::Copied
        } else if explicit_added || (old_path.is_none() && new_path.is_some()) {
            FileStatus::Added
        } else if explicit_deleted || (new_path.is_none() && old_path.is_some()) {
            FileStatus::Deleted
        } else {
            FileStatus::Modified
        };
        file.old_path = old_path;
        file.new_path = new_path;
        self.files.push(file);
    }

    /// Skip `GIT binary patch` payload: data lines until a blank line or the
    /// next file header. (The second `literal`/`delta` section after the
    /// blank line is consumed as top-level junk, which is fine.)
    fn skip_binary_body(&mut self) {
        while self.pos < self.lines.len() {
            let line = self.current();
            if line.is_empty() || line.starts_with(b"diff --git ") {
                break;
            }
            self.pos += 1;
        }
    }

    fn parse_hunks(&mut self, file: &mut FileDiff) {
        while let Some(mut hunk) = parse_hunk_header(self.current()) {
            self.pos += 1;
            self.parse_hunk_body(&mut hunk);
            file.hunks.push(hunk);
        }
    }

    fn parse_hunk_body(&mut self, hunk: &mut Hunk) {
        let mut rem_old = hunk.old_count as u64;
        let mut rem_new = hunk.new_count as u64;

        while rem_old > 0 || rem_new > 0 {
            if self.pos >= self.lines.len() {
                self.warn("hunk truncated by end of input");
                return;
            }
            let line = self.current();
            let (kind, uses_old, uses_new) = match line.first() {
                Some(b' ') | None => (LineKind::Context, true, true),
                Some(b'-') => (LineKind::Deletion, true, false),
                Some(b'+') => (LineKind::Addition, false, true),
                Some(b'\\') => {
                    if let Some(last) = hunk.lines.last_mut() {
                        last.no_newline = true;
                    }
                    self.pos += 1;
                    continue;
                }
                Some(_) => {
                    self.warn("unexpected line inside hunk; hunk truncated");
                    return;
                }
            };
            if (uses_old && rem_old == 0) || (uses_new && rem_new == 0) {
                self.warn("hunk has more lines than its header declares");
                return;
            }
            if uses_old {
                rem_old -= 1;
            }
            if uses_new {
                rem_new -= 1;
            }
            hunk.lines.push(Line {
                kind,
                content: line.get(1..).unwrap_or(b"").to_vec(),
                no_newline: false,
            });
            self.pos += 1;
        }

        // A final `\ No newline at end of file` after the counted lines.
        if self.current().starts_with(b"\\") {
            if let Some(last) = hunk.lines.last_mut() {
                last.no_newline = true;
            }
            self.pos += 1;
        }

        // Overlong hunks: a further `+`/`-` line that is not the next file's
        // `---`/`+++` marker means the header undercounted. Warn without
        // consuming — the line may still be meaningful junk to skip upstream.
        let next = self.current();
        if matches!(next.first(), Some(b'+') | Some(b'-'))
            && !next.starts_with(b"+++ ")
            && !next.starts_with(b"--- ")
        {
            self.warn("hunk has more lines than its header declares");
        }
    }
}

/// Parse `@@ -old[,count] +new[,count] @@[ heading]`.
fn parse_hunk_header(line: &[u8]) -> Option<Hunk> {
    let rest = line.strip_prefix(b"@@ -")?;
    let mut cur = Cursor { s: rest, i: 0 };
    let (old_start, old_count) = cur.range()?;
    cur.eat(b" +")?;
    let (new_start, new_count) = cur.range()?;
    cur.eat(b" @@")?;
    let heading = match cur.remainder() {
        [] => None,
        [b' ', h @ ..] => Some(h.to_vec()),
        other => Some(other.to_vec()),
    };
    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        heading,
        lines: Vec::new(),
    })
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    /// `start[,count]` — count defaults to 1 when omitted.
    fn range(&mut self) -> Option<(u32, u32)> {
        let start = self.number()?;
        let count = if self.eat(b",").is_some() {
            self.number()?
        } else {
            1
        };
        Some((start, count))
    }

    fn number(&mut self) -> Option<u32> {
        let digits = self
            .s
            .get(self.i..)?
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits == 0 {
            return None;
        }
        let mut value: u32 = 0;
        for &b in self.s.get(self.i..self.i + digits)? {
            value = value.saturating_mul(10).saturating_add(u32::from(b - b'0'));
        }
        self.i += digits;
        Some(value)
    }

    fn eat(&mut self, token: &[u8]) -> Option<()> {
        if self.s.get(self.i..)?.starts_with(token) {
            self.i += token.len();
            Some(())
        } else {
            None
        }
    }

    fn remainder(&self) -> &[u8] {
        self.s.get(self.i..).unwrap_or(b"")
    }
}

/// Extract a path from a `--- ` / `+++ ` marker line.
/// Returns `None` for `/dev/null`. Strips git's `a/`/`b/` prefixes when
/// `git_prefixes` is set; truncates at the first tab (plain `diff -u`
/// appends a timestamp, git appends a tab for paths with trailing spaces).
fn marker_path(line: &[u8], marker: &[u8], git_prefixes: bool) -> Option<ByteStr> {
    let value = line.strip_prefix(marker)?;
    let path = unquote_value(value);
    if path == b"/dev/null" {
        return None;
    }
    let path = if git_prefixes {
        strip_ab_prefix(&path).to_vec()
    } else {
        path
    };
    Some(path)
}

fn strip_ab_prefix(path: &[u8]) -> &[u8] {
    path.strip_prefix(b"a/")
        .or_else(|| path.strip_prefix(b"b/"))
        .unwrap_or(path)
}

/// A header value that may be C-quoted (`"caf\303\251.md"`); unquoted values
/// are truncated at the first tab.
fn unquote_value(value: &[u8]) -> ByteStr {
    if let Some(inner) = quoted_span(value) {
        return c_unquote(inner);
    }
    let end = value
        .iter()
        .position(|&b| b == b'\t')
        .unwrap_or(value.len());
    value.get(..end).unwrap_or(value).to_vec()
}

/// If `s` starts with a double quote, return the span inside the matching
/// closing quote (escapes respected).
fn quoted_span(s: &[u8]) -> Option<&[u8]> {
    let inner = s.strip_prefix(b"\"")?;
    let mut i = 0;
    while i < inner.len() {
        match inner.get(i) {
            Some(b'\\') => i += 2,
            Some(b'"') => return inner.get(..i),
            _ => i += 1,
        }
    }
    None // unterminated quote: treat as unquoted
}

/// Decode git's C-style quoting: `\n \t \r \a \b \f \v \\ \" \ooo`.
/// Unknown escapes are kept literally (tolerance over strictness).
fn c_unquote(s: &[u8]) -> ByteStr {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while let Some(&b) = s.get(i) {
        if b != b'\\' {
            out.push(b);
            i += 1;
            continue;
        }
        match s.get(i + 1) {
            Some(b'n') => out.push(b'\n'),
            Some(b't') => out.push(b'\t'),
            Some(b'r') => out.push(b'\r'),
            Some(b'a') => out.push(0x07),
            Some(b'b') => out.push(0x08),
            Some(b'f') => out.push(0x0c),
            Some(b'v') => out.push(0x0b),
            Some(b'\\') => out.push(b'\\'),
            Some(b'"') => out.push(b'"'),
            Some(&d) if (b'0'..=b'7').contains(&d) => {
                let mut value: u32 = 0;
                let mut len = 0;
                while len < 3 {
                    match s.get(i + 1 + len) {
                        Some(&o) if (b'0'..=b'7').contains(&o) => {
                            value = value * 8 + u32::from(o - b'0');
                            len += 1;
                        }
                        _ => break,
                    }
                }
                out.push((value & 0xff) as u8);
                i += 1 + len;
                continue;
            }
            Some(&other) => {
                out.push(b'\\');
                out.push(other);
            }
            None => out.push(b'\\'),
        }
        i += 2;
    }
    out
}

/// Provisional paths from `diff --git a/X b/Y`. Ambiguous when paths contain
/// spaces and are unquoted; the authoritative `rename from/to` or `---`/`+++`
/// lines override these whenever present.
fn git_header_paths(rest: &[u8]) -> (Option<ByteStr>, Option<ByteStr>) {
    if rest.starts_with(b"\"") {
        let Some(old_inner) = quoted_span(rest) else {
            return (None, None);
        };
        let old = c_unquote(old_inner);
        let after = rest.get(old_inner.len() + 2..).unwrap_or(b"");
        let new_raw = after.strip_prefix(b" ").unwrap_or(after);
        let new = match quoted_span(new_raw) {
            Some(inner) => c_unquote(inner),
            None => new_raw.to_vec(),
        };
        return (
            Some(strip_ab_prefix(&old).to_vec()),
            Some(strip_ab_prefix(&new).to_vec()),
        );
    }
    // Unquoted: split at the rightmost ` b/`.
    let split = (0..rest.len().saturating_sub(2))
        .rev()
        .find(|&i| rest.get(i..i + 3) == Some(b" b/"));
    match split {
        Some(i) => {
            let old = rest.get(..i).unwrap_or(b"");
            let new = rest.get(i + 1..).unwrap_or(b"");
            (
                Some(strip_ab_prefix(old).to_vec()),
                Some(strip_ab_prefix(new).to_vec()),
            )
        }
        None => (None, None),
    }
}

fn parse_octal(s: &[u8]) -> Option<u32> {
    let digits: Vec<u8> = s
        .iter()
        .take_while(|b| (b'0'..=b'7').contains(b))
        .copied()
        .collect();
    if digits.is_empty() {
        return None;
    }
    let mut value: u32 = 0;
    for b in digits {
        value = value.saturating_mul(8).saturating_add(u32::from(b - b'0'));
    }
    Some(value)
}

fn parse_percent(s: &[u8]) -> Option<u8> {
    let digits = s.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let mut value: u32 = 0;
    for &b in s.get(..digits)? {
        value = value.saturating_mul(10).saturating_add(u32::from(b - b'0'));
    }
    Some(value.min(100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_clean(input: &[u8]) -> Changeset {
        let outcome = parse_unified(input);
        assert_eq!(outcome.warnings, vec![], "unexpected warnings");
        outcome.changeset
    }

    #[test]
    fn hunk_header_variants() {
        let full = parse_hunk_header(b"@@ -1,5 +2,6 @@ fn main()");
        assert_eq!(
            full,
            Some(Hunk {
                old_start: 1,
                old_count: 5,
                new_start: 2,
                new_count: 6,
                heading: Some(b"fn main()".to_vec()),
                lines: Vec::new(),
            })
        );
        // Counts default to 1; no heading.
        let short = parse_hunk_header(b"@@ -3 +4 @@");
        assert_eq!(
            short.map(|h| (
                h.old_start,
                h.old_count,
                h.new_start,
                h.new_count,
                h.heading
            )),
            Some((3, 1, 4, 1, None))
        );
        assert_eq!(parse_hunk_header(b"@@ garbage @@"), None);
        assert_eq!(parse_hunk_header(b"not a hunk"), None);
    }

    #[test]
    fn c_unquote_decodes_octal_and_escapes() {
        assert_eq!(c_unquote(br#"caf\303\251 \"x\"\t"#), b"caf\xc3\xa9 \"x\"\t");
        // Unknown escape kept literally; trailing backslash kept.
        assert_eq!(c_unquote(br"a\qb\"), br"a\qb\");
    }

    #[test]
    fn fully_crlf_patch_is_normalized() {
        let lf: &[u8] = b"--- a.txt\n+++ b.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let crlf: Vec<u8> = lf
            .iter()
            .flat_map(|&b| {
                if b == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![b]
                }
            })
            .collect();
        assert_eq!(parse_unified(&crlf), parse_unified(lf));
    }

    #[test]
    fn crlf_content_bytes_are_preserved() {
        let patch = b"--- a.txt\n+++ b.txt\n@@ -1,1 +1,1 @@\n-old\r\n+new\r\n";
        let cs = parse_clean(patch);
        let lines = &cs.files[0].hunks[0].lines;
        assert_eq!(lines[0].content, b"old\r".to_vec());
        assert_eq!(lines[1].content, b"new\r".to_vec());
    }

    #[test]
    fn empty_context_lines_count_as_context() {
        // Mail-mangled patches strip the trailing space of empty context lines.
        let patch = b"--- a.txt\n+++ b.txt\n@@ -1,3 +1,3 @@\n x\n\n-y\n+z\n";
        let cs = parse_clean(patch);
        let hunk = &cs.files[0].hunks[0];
        assert_eq!(hunk.lines.len(), 4);
        assert_eq!(hunk.lines[1].kind, LineKind::Context);
        assert_eq!(hunk.lines[1].content, b"".to_vec());
    }

    #[test]
    fn truncated_hunk_warns_instead_of_failing() {
        let patch = b"--- a.txt\n+++ b.txt\n@@ -1,5 +1,5 @@\n ctx\n";
        let outcome = parse_unified(patch);
        assert_eq!(outcome.changeset.files.len(), 1);
        assert_eq!(outcome.changeset.files[0].hunks[0].lines.len(), 1);
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].message.contains("truncated"));
    }

    #[test]
    fn overlong_hunk_warns() {
        let patch = b"--- a.txt\n+++ b.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n+extra\n";
        let outcome = parse_unified(patch);
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].message.contains("more lines"));
    }

    #[test]
    fn garbage_input_yields_empty_changeset_without_panic() {
        for input in [
            &b"hello world"[..],
            &b"\xff\xfe\x00\x01\x02"[..],
            &b"diff --git\n@@ -\n--- \n+++ \n\\\n"[..],
            &b"@@ -1,1 +1,1 @@\n"[..],
            &b""[..],
        ] {
            let outcome = parse_unified(input);
            assert!(outcome.changeset.files.is_empty(), "input: {input:?}");
        }
    }

    #[test]
    fn git_header_with_quoted_paths() {
        let (old, new) = git_header_paths(br#""a/caf\303\251 plan.md" "b/caf\303\251 plan.md""#);
        assert_eq!(old, Some("café plan.md".as_bytes().to_vec()));
        assert_eq!(new, Some("café plan.md".as_bytes().to_vec()));
    }
}
