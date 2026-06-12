//! Strip ANSI escape sequences from patch bytes.
//!
//! Git colorizes output sent to a pager (`color.ui = auto` treats the pager
//! as a terminal), so `margin pager` receives `\x1b[32m+added\x1b[m` style
//! bytes. Parsing needs them gone; the byte-identical pager passthrough
//! (ADR-0007) does NOT use this — raw bytes flow through untouched there.
//!
//! Handles CSI (`ESC [ ... final`), OSC (`ESC ] ... BEL`/`ESC \`), and
//! two-byte escapes; panic-free on truncated sequences (ADR-0009).

/// Remove ANSI escape sequences, preserving all other bytes exactly.
pub fn strip_ansi(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while let Some(&byte) = input.get(i) {
        if byte != 0x1b {
            out.push(byte);
            i += 1;
            continue;
        }
        match input.get(i + 1) {
            // CSI: parameters/intermediates, then one final byte in 0x40..=0x7e.
            Some(b'[') => {
                let mut j = i + 2;
                while let Some(&b) = input.get(j) {
                    j += 1;
                    if (0x40..=0x7e).contains(&b) {
                        break;
                    }
                }
                i = j;
            }
            // OSC: until BEL or the ESC \ string terminator.
            Some(b']') => {
                let mut j = i + 2;
                loop {
                    match input.get(j) {
                        None => break,
                        Some(0x07) => {
                            j += 1;
                            break;
                        }
                        Some(0x1b) if input.get(j + 1) == Some(&b'\\') => {
                            j += 2;
                            break;
                        }
                        Some(_) => j += 1,
                    }
                }
                i = j;
            }
            // Other two-byte escapes (ESC c, ESC =, ...).
            Some(_) => i += 2,
            // Lone trailing ESC: drop it.
            None => i += 1,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn colored_git_diff_line_becomes_plain() {
        let colored = b"\x1b[32m+let a = 1;\x1b[m\n\x1b[31m-let a = 0;\x1b[m\n";
        assert_eq!(strip_ansi(colored), b"+let a = 1;\n-let a = 0;\n");
    }

    #[test]
    fn plain_bytes_pass_through_unchanged() {
        let plain = b"diff --git a/x b/x\n+\xff\xfeinvalid utf8 kept\n";
        assert_eq!(strip_ansi(plain), plain.to_vec());
    }

    #[test]
    fn osc_sequences_and_truncated_escapes_do_not_panic() {
        assert_eq!(strip_ansi(b"\x1b]0;title\x07text"), b"text");
        assert_eq!(strip_ansi(b"\x1b]8;;url\x1b\\link"), b"link");
        assert_eq!(strip_ansi(b"text\x1b["), b"text");
        assert_eq!(strip_ansi(b"text\x1b"), b"text");
        assert_eq!(strip_ansi(b"\x1b]unterminated"), b"");
    }

    #[test]
    fn colored_patch_parses_like_plain() {
        let plain = b"--- a.txt\n+++ b.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let colored =
            b"\x1b[1m--- a.txt\x1b[m\n\x1b[1m+++ b.txt\x1b[m\n\x1b[36m@@ -1,1 +1,1 @@\x1b[m\n\x1b[31m-old\x1b[m\n\x1b[32m+new\x1b[m\n";
        assert_eq!(
            crate::parse_unified(&strip_ansi(colored)),
            crate::parse_unified(plain)
        );
    }
}
