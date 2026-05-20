# pkms fix

Replace a broken full UUID in `id:` links with a replacement full UUID.

```bash
pkms fix <broken-full-uuid> <replacement-full-uuid>
pkms fix <broken-full-uuid> <replacement-full-uuid> --apply
```

`fix` is a dry run unless `--apply` is present. Both arguments must be valid
full UUIDs with dashes, and the replacement UUID must exist in the database.

## Safe Workflow

1. Run `pkms check --id-links`.
2. Inspect each source note and read the link description.
3. Resolve the intended replacement note with `resolve` or `query`.
4. Dry-run `fix`.
5. Apply only when every occurrence should map to the same replacement.
6. Run `pkms check --self-links --id-links`.

If the same broken UUID appears in different semantic contexts, edit links
manually instead of using `fix`.
