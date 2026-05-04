# pkms path — Find Shortest Path Between Two Notes

## When to Use

Use `path` to find the shortest connection between two notes through the directed graph. This is useful for understanding how two topics are related, discovering intermediate concepts, and navigating the knowledge graph.

## How It Works

1. Resolves both source and target notes (by UUID, file path, or title)
2. Runs a bidirectional BFS (breadth-first search) through the graph, traversing both outgoing and incoming links
3. Returns the shortest path if one exists

If no path exists (disconnected components), the command reports "No path found" and exits successfully.

## Arguments

| Arg | Description |
|-----|-------------|
| `from` | Source note (UUID, file path, or title) |
| `to` | Target note (UUID, file path, or title) |

## Output

### Text — path found

```
Shortest path between "Note A" and "Note C":
  2 hop(s)

  1. Note A
  2. Note B
  3. Note C
```

### Text — no path

```
No path found between "Note A" and "Isolated Note"
```

### JSON — path found

```json
{
  "from": "Note A",
  "to": "Note C",
  "found": true,
  "hops": 2,
  "path": [
    {"uuid": "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa", "title": "Note A"},
    {"uuid": "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb", "title": "Note B"},
    {"uuid": "cccccccc-cccc-4ccc-cccc-cccccccccccc", "title": "Note C"}
  ]
}
```

### JSON — no path

```json
{
  "from": "Note A",
  "to": "Isolated",
  "found": false,
  "hops": 0,
  "path": []
}
```

## Typical Scenarios

### Find connection between two known topics
```bash
pkms path "Machine Learning" "Clojure"
```

### Machine-readable for analysis
```bash
pkms --output-format json path "Note A" "Note C"
```

## Performance Notes

- BFS visits every node in the worst case; on large databases (~1500+ notes) it completes in milliseconds
- The search traverses both forward and backward links bidirectionally for efficiency
