# pkms info — Show Resolved Configuration

## When to Use

Use `info` to see how `pkms` resolved its configuration — which config file was loaded, the effective database root, the new notes directory, and whether `--db` was used to override.

## How It Works

`info` reads the config (if present), resolves the database root from config or `--db` flag, resolves the new notes directory, and displays the final effective settings.

## Output

### Text (default)

```
pkms configuration
  config file:  ~/.config/pkms.toml (found)
  db_root:      /home/user/Documents/org
  new_notes:    /home/user/Documents/org/roam
  (db_root overridden via --db flag)
```

### JSON

```json
{
  "config": {
    "db_root": "/home/user/Documents/org",
    "new_notes_dir": "/home/user/Documents/org/roam",
    "ignore_patterns": [".attach", "*.bak"],
    "has_config_file": true
  },
  "config_path": "/home/user/.config/pkms.toml",
  "cli_overrides": {
    "db_override": true
  }
}
```

## Typical Scenarios

### Check current config
```bash
pkms info
```

### Verify --db override
```bash
pkms --db /custom/path info
```

### Machine-readable
```bash
pkms --output-format json info
```
