# pkms fix — Replace Broken UUIDs Across Database

## When to Use

After running `check` and identifying broken UUIDs, use `fix` to replace all occurrences of a broken UUID across every `.org` file in the database with a valid replacement.

## How It Works

1. Loads the full graph to resolve the replacement target (by title, UUID, or UUID prefix)
2. Scans all `.org` files in the database root for occurrences of the broken UUID string
3. Performs a dry-run by default — shows what would change without modifying files
4. With `--apply`, performs the actual string replacement in every affected file

The broken UUID can be specified as a full UUID (with dashes) or as an 8-character prefix (resolved against known broken UUIDs from `Graph::broken_links`).

The replacement target can be a note title, full UUID, or a unique 8-character prefix.

## Arguments & Flags

| Arg/Flag | Description |
|----------|-------------|
| `broken_uuid` | The broken UUID to replace (full or 8-char prefix) |
| `target` | Replacement: note title, UUID, or unique prefix |
| `-a`, `--apply` | Actually apply the fix (without this, dry-run only) |

## Output

### Text — dry-run

```
Would fix 3 broken link(s) in 2 file(s):
  Broken UUID: ffffffff-ffff-4fff-ffff-ffffffffffff
  Replace with: Note A (aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa)
  (use --apply to apply)
  /org/roam/common/broken.org
  /org/roam/personal/another.org
```

### Text — applied

```
Fixed 3 broken link(s) in 2 file(s):
  /org/roam/common/broken.org
  /org/roam/personal/another.org
```

### JSON

```json
{
  "broken_uuid": "ffffffff-ffff-4fff-ffff-ffffffffffff",
  "replacement_uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
  "replacement_title": "Note A",
  "files_affected": ["/org/roam/common/broken.org"],
  "total_replacements": 3,
  "applied": false
}
```

## Typical Scenarios

### Dry-run to see what would change
```bash
pkms fix "ffffffff-ffff-4fff-ffff-ffffffffffff" "Note A"
```

### Actually apply the fix
```bash
pkms fix "ffffffff-ffff-4fff-ffff-ffffffffffff" "Note A" --apply
```

### Use short prefix
```bash
pkms fix "ffffffff" "Note A" --apply
```

## Safety Notes

- Always run without `--apply` first to see what would change
- After applying, run `pkms check` to verify the fix resolved the issue
- The replacement string is a plain UUID string replacement — it will replace ANY occurrence of the broken UUID string in `.org` files, including in non-link contexts. Verify the dry-run output carefully.
