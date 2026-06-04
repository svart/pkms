# Notes Database Format

`pkms` expects an org-roam directory containing `.org` notes.

## Notes

Each note should have:

- A UUID v4 `:ID:` property.
- A `#+title:` keyword.
- Optional `#+filetags:` in canonical colon form.
- Optional `:ROAM_ALIASES:` property.

Example:

```org
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:ROAM_ALIASES: "Alias One" "Alias Two"
:END:
#+title: Example Note
#+filetags: :example:reference:
```

## Links

Internal links use org-roam IDs:

```org
[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa][Example Note]]
```

`pkms` also recognizes `file:`, URL, and `attachment:` links. Health checks can
validate local `file:` and `attachment:` targets.

## Heading Nodes

Headings with `:ID:` properties become first-class graph nodes. This allows
direct links to a section inside a note.

```org
* HTTP
** HTTP/2
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
```

Generate heading IDs with:

```bash
pkms new "Existing Note" --create --heading "HTTP/2"
```

Inspect heading IDs with:

```bash
pkms get <target> --headings --no-content
```

Retrieve one heading block from a note with:

```bash
pkms get <target> --heading "HTTP/2"
```

## Filetags

Use canonical filetags:

```org
#+filetags: :tag1:tag2:
```

Run `pkms check --filetags` to find malformed filetags.
