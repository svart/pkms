# pkms check

Use `check` for whole-database health. It loads the graph and reports issues.
Exit code is nonzero when health issues are found.

## Focused Checks

```bash
pkms check
pkms check --stats
pkms check --id-links
pkms check --file-links
pkms check --remote-file-links
pkms check --attachment-links
pkms check --filetags
pkms check --self-links
pkms check --overlinks
pkms check --cross-links <note-a> <note-b>
```

When no section flags are given, all default checks are shown. When section
flags are given, output is limited to those sections.

`check --stats` reports orphan counts with the same policy as default
`pkms orphans`: daily notes are excluded.

`check --remote-file-links` requires a build with `--features ssh`. It checks
TRAMP-style SSH `file:` links explicitly and reports remote auth, host-key,
timeout, unsupported syntax, and SFTP failures in `file_link_errors`. Without
that flag, SSH `file:` links are skipped and no network access is attempted.
SSH checks use the user's standard non-interactive public-key setup
(`~/.ssh/known_hosts`, standard identity files, and SSH agent); there is no
`[ssh]` config block.

## Agent Workflow

- Use `--id-links` before fixing broken UUIDs.
- Use `--self-links` after broad UUID replacement.
- Use `--filetags` after changing note headers.
- Use `--remote-file-links` only when remote SSH checks are intended.
- Use JSON for automation: `pkms --output-format json check --id-links`.
