# JSON output (`--json`)

`margin diff --json`, `margin show --json`, and `margin patch --json` emit
the parsed changeset as a single JSON document on stdout — how agents and
scripts consume Margin without scraping a terminal.

```bash
margin diff --json | jq '.files[] | {path: .new_path, adds: .additions}'
git show HEAD --format= | margin patch --json - | jq '.additions'
```

`pager` mode never emits JSON: its piped output is byte-identical to its
input by contract (ADR-0007). `--json` cannot combine with `--watch`.

## Stability

The document carries `"schema": 1`. Within schema 1, changes are
**additive only** (new optional fields may appear; nothing is removed or
retyped). A breaking change would ship as `"schema": 2` — consumers should
check the field.

## Encoding

Margin's model is bytes-first; JSON strings are UTF-8. Every string in the
document is **lossy UTF-8**: invalid bytes become U+FFFD (`�`), and any
value that was altered carries a sibling flag — `"lossy": true` on lines,
`"path_lossy": true` on files. The flags are omitted when false. Consumers
needing exact bytes should read the raw diff itself; the JSON view is for
structure.

## Schema 1

```jsonc
{
  "schema": 1,
  "files": [
    {
      "status": "modified",        // added | deleted | modified | renamed | copied
      "old_path": "src/app.rs",    // null for added files
      "new_path": "src/app.rs",    // null for deleted files
      "path_lossy": true,          // OMITTED unless a path had invalid UTF-8
      "binary": false,             // binary files carry no hunks
      "old_mode": "100644",        // octal string, git-style; null when unknown
      "new_mode": "100755",
      "similarity": 90,            // OMITTED except for renames/copies with an index
      "additions": 3,              // line counts for this file
      "deletions": 1,
      "hunks": [
        {
          "old_start": 1, "old_count": 5,
          "new_start": 1, "new_count": 6,
          "heading": "fn main()",  // the @@ ... @@ section text; OMITTED when absent
          "lines": [
            {
              "kind": "context",   // context | addition | deletion
              "content": "…",      // no +/-/space marker, no trailing newline
              "lossy": true,       // OMITTED unless content had invalid UTF-8
              "no_newline": true   // OMITTED unless "\ No newline at end of file"
            }
          ]
        }
      ]
    }
  ],
  "additions": 3,                  // totals across all files
  "deletions": 1
}
```

The producing types live in `margin-core/src/json.rs` — a deliberate
public surface, decoupled from the internal model so refactors cannot
change this document shape silently.
