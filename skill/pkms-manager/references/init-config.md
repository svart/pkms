# pkms init-config — Generate Default Config File

## When to Use

Use `init-config` on first setup to generate `~/.config/pkms.toml` with default settings. Optionally pre-fill the database root path.

## How It Works

Creates `~/.config/pkms.toml` if it does not already exist. Errors if the file already exists (to prevent overwriting). The generated file has commented-out defaults with an optional `db_root` value.

## Flags

| Flag | Description |
|------|-------------|
| `-d`, `--db PATH` | Database root path to write into the config |

## Output

### Text (default)

```
Created config at /home/user/.config/pkms.toml
```

### JSON

```json
{"created": "/home/user/.config/pkms.toml"}
```

## Generated Config

```toml
# pkms configuration
# db_root = "/path/to/your/org/directory"

# Directory where new notes are created (relative to db_root or absolute)
# new_notes_dir = "roam"

# Glob patterns to ignore during file discovery
# ignore_patterns = [".attach", "*.bak"]
```

## Typical Scenarios

### First-time setup
```bash
pkms init-config
```

### With database root pre-filled
```bash
pkms init-config --db ~/Documents/org
```

## Error Handling

- Errors if `~/.config/pkms.toml` already exists (use `rm ~/.config/pkms.toml` first to regenerate)
- Errors if XDG config directory cannot be found
