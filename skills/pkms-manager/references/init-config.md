# pkms init-config

Create `~/.config/pkms.toml` if it does not already exist.

```bash
pkms init-config
pkms init-config --db ~/Documents/org
pkms --output-format json init-config --db ~/Documents/org
```

The command fails rather than overwriting an existing config.
