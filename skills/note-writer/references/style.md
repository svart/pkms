# Style Reference

Use this reference when drafting or revising note prose.

## Opening Definition Pattern

Every core-concept note opens with a tight two-sentence definition. The first sentence categorizes what the thing is. The second draws a boundary: what makes it distinct, what its scope is, or what it is not.

Good openings:

> A widget is a mechanism that transforms input into output. Unlike filters, a widget preserves the full structure.

> Frobnication is a technique for rearranging data in place. It changes the order of existing data rather than creating new data.

Structure: sentence one gives category and essential action; sentence two gives boundary or contrast. Both sentences must stand alone. Adjust the formula when clarity demands it.

## Second Paragraph

The second paragraph expands the opening without digression. It stays at the same level of abstraction and deepens the first claim.

> A widget works by accepting an input stream and producing a transformed output. Its purpose is to guarantee that no information is lost during the process. Unlike filters, widgets preserve the full structure of the input. A widget is a transducer.

Do not introduce a new topic in this paragraph.

## Sentence Rhythm

Use short, declarative sentences. Each sentence should add one new piece of information. Avoid dependent clauses that delay the main point.

Prefer:

> A widget is a data transformer. In a transformation, what matters is the mapping from input to output. By contrast, the internal state of the widget is irrelevant to the caller.

Avoid:

> A widget, which is a data transformer that produces an output from an input, is something in which the internal state is irrelevant to the caller.

## Headings

Use consecutive org heading levels. If the parent is `*`, the child is `**`, not `***`. A `***` heading is valid only under a preceding `**`.

Use org headings for sections:

```org
* Section Name
** Subsection Name
```

Never use a standalone bold line as a section heading. Replace `*Section Name*` with `* Section Name`.

## Heading IDs

When a heading covers a concept that deserves its own link anchor, add a properties drawer immediately after the heading:

```org
** HTTP/2
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
```

Generate heading UUIDs with:

```bash
pkms new "Note Title" --create --heading "HTTP/2"
```

Link to the heading with:

```org
[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][HTTP/2]]
```

Add heading IDs only when another note is likely to link directly to that section. Good candidates are distinct protocols, versions, variants, standards, or sub-concepts inside a broader note.

## Principle Lists

When a source lists principles, rules, or steps, introduce each as an imperative or directive subheading and follow it with a short explanatory paragraph:

```org
*** Maintain idempotency
Ensure the widget produces the same result when given the same input multiple times. This makes the system predictable and testable.
```

Do not use numbered steps unless the source itself uses them and order matters.

## Bold Text

Use org-mode `*bold*` sparingly for definitional anchors:

```org
*Idempotency* is the property of producing the same result on repeated application.
```

Do not bold for emphasis. Do not use bold text as a substitute for headings.

## Analogies

An optional analogy may end a core note when it gives real clarity. Use plain, concrete scenarios rather than decorative metaphors or culturally dependent idioms. If the analogy feels forced, omit it.

When used, state the domain, describe the scenario, and let the parallel emerge from the prose.

## Quotes

Use org block quotes for short authoritative quotations:

```org
#+begin_quote
A widget is only as good as its failure modes.

-- Some Authority
#+end_quote
```

Quotes should be short and self-contained. Do not explain a quotation immediately after quoting it unless the source requires interpretation.

## Code And Verbatim

Use `~...~` for short non-code technical terms, notation, paths, and verbatim text: `~O(n)~`, `~rpm~`, `~/var/log~`, `~:ID:~`.

Use `=...=` for inline source code only: `=function_call(2)=`, `=std::vector<int>=`.

Use source blocks for multi-line examples:

```org
#+begin_src python
def greet(name):
    print(f"hello, {name}")
#+end_src
```

Shell examples use `sh` or `shell`. Keep each block focused on one concept.

## Tables

Use org tables with short noun-phrase headers:

```org
| Metric | Meaning                | Scope        |
|--------+------------------------+--------------|
| RSSI   | Received signal power  | Link quality |
| SNR    | Signal-to-noise ratio  | Signal clean |
```

Keep tables narrow. If a table needs many columns, split it or convert it to prose.

## Voice

Write in a neutral, authoritative, plain register. Use third person except in direct quotes. Prefer present simple for timeless claims. The voice is a knowledgeable practitioner explaining to a peer.

Avoid:

- Rhetorical questions.
- First-person editorializing.
- Humor, idioms, or decorative metaphors.
- Bullets where prose works better.
- Empty transitions such as "It is important to note that".
- The repeated frame "It is not X, it is Y"; state the positive claim directly.
