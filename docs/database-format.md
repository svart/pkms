# Notes Database Format

`pkms` expects an org-roam directory containing `.org` notes.

## Notes

Each note should have:

- A UUID v4 `:ID:` property.
- A `#+title:` keyword.
- Optional `#+filetags:` in canonical colon form.
- Optional `:ROAM_ALIASES:` property.
- Optional `:ROAM_REFS:`, `:CATEGORY:`, and `:PROJECT:` properties.

Example:

```org
:PROPERTIES:
:ID:       aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
:ROAM_ALIASES: "Alias One" "Alias Two"
:ROAM_REFS: https://example.com/book
:CATEGORY: Reference
:PROJECT: Example Project
:END:
#+title: Example Note
#+filetags: :example:reference:
```

Aliases participate in title lookup. `ROAM_REFS` participate in query matches.
`CATEGORY` values are included with tag/category-style lookup, and `PROJECT`
properties are used by task metadata and project filters.

## Links

Internal links use org-roam IDs:

```org
[[id:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa][Example Note]]
```

`pkms` also recognizes `file:`, URL, and `attachment:` links. Health checks can
validate local `file:` and `attachment:` targets.

SSH `file:` links may use a TRAMP-style target when `pkms` is built with the
`ssh` feature and `check --remote-file-links` is requested:

```org
[[file:/ssh:host:/absolute/path]]
[[file:/ssh:user@host:/absolute/path]]
[[file:/ssh:user@host#222:/absolute/path]]
```

Remote checks are not run by default. Without `--remote-file-links`, SSH file
targets are skipped by `check` and `validate` instead of being treated as local
absolute paths.

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

## Task Headings

Task headings are org headings with configured TODO states. They may include
priority, heading tags, `SCHEDULED`, `DEADLINE`, and heading-level `PROJECT`
properties:

```org
* TODO [#A] Call Alice :phone:
SCHEDULED: <2026-06-05 Fri>
:PROPERTIES:
:PROJECT: Example Project
:END:
```

The configured open and closed TODO states control which headings are treated as
tasks and how canonical task IDs are assigned.
