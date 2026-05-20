# pkms open

Open a task or note in an editor.

```bash
pkms open <canonical-id>
pkms open <uuid-or-title>
pkms open <target> --line 42
pkms open <target> --editor "emacsclient -n"
```

Numeric targets are canonical task IDs from `todo` and `agenda`. Non-numeric
targets can be UUIDs, paths, or note titles.

Default editor command is `emacsclient -n`.
