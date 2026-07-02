# pkms fix

Repair broken `id:` link UUIDs or misplaced heading-scoped attachment files.

```bash
pkms fix uuid <broken-full-uuid> <replacement-full-uuid>
pkms fix uuid <broken-full-uuid> <replacement-full-uuid> --apply
pkms fix attach
pkms fix attach --apply
pkms fix attach --apply --copy
```

`fix uuid` is a dry run unless `--apply` is present. Both arguments must be
valid full UUIDs with dashes, and the replacement UUID must exist in the
database.

`fix attach` is also a dry run unless `--apply` is present. It inspects
`attachment:` links inside Org headings, computes the org-attach directory for
the heading ID or note ID fallback, and repairs a missing target only when
exactly one matching file is found under the supported `.attach` roots. It does
not rewrite Org link text. Pass `--copy` with `--apply` to copy instead of move.

## Safe Workflow

1. Run `pkms check --id-links`.
2. Inspect each source note and read the link description.
3. Resolve the intended replacement note with `resolve` or `query`.
4. Dry-run `fix uuid`.
5. Apply only when every occurrence should map to the same replacement.
6. Run `pkms check --self-links --id-links`.

If the same broken UUID appears in different semantic contexts, edit links
manually instead of using `fix`.
