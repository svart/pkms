---
name: note-writer
description: Write well-structured, stylistically consistent org-mode notes for an org-roam PKMS database. Use this skill to create a cohesive set of notes on any topic, following a proven pattern for content structure, phrasing, and cross-linking. Use the pkms-manager skill to find existing note UUIDs for internal links.
---

# Note-writer

This skill helps you write a cohesive set of org-mode notes on any topic, following a consistent style for content structure, phrasing, and cross-linking.

## Prerequisites

Before writing notes, resolve existing note UUIDs using the `pkms-manager` skill:

```bash
pkms resolve --title <term>
pkms query "term"
```

Use the returned UUIDs to create inline links between notes.

## Writing Style Reference

### Opening Definition Pattern

Every core-concept note opens with a tight two-sentence definition. The first sentence categorises what the thing *is*. The second draws a boundary — it states what makes it distinct, what its scope is, or what it is not. Both sentences are complete, self-contained statements. Never use an em-dash to tack a second-person explanation onto the second sentence:

> A widget is a mechanism that transforms input into output. Unlike filters, a widget preserves the full structure.

> Frobnication is a technique for rearranging data in place. It does not create new data, only changes the order.

> A quux is a formal description of the system's behaviour. It specifies what the system does, not how to use it.

**Structure**: sentence 1 = category + essential action. sentence 2 = boundary or contrast that distinguishes it from similar concepts. Both sentences must stand on their own. Specific formular could be changed in favor of clarity.

### Second-Paragraph Elaboration

Immediately after the opening, a second paragraph expands without digression — more detail on the same thought, same register, same level of abstraction:

> A widget works by accepting an input stream and producing a transformed output. Its purpose is to guarantee that no information is lost during the process. Unlike filters, widgets preserve the full structure of the input. A widget is a transducer.

This paragraph never introduces a new topic. It deepens the opening claim.

### Sentence Rhythm

Short, declarative sentences. Compound sentences joined by commas, not semicolons. Each sentence adds exactly one new piece of information. Avoid dependent clauses that delay the main point.

**Prefer**:
> A widget is a data transformer. In a transformation, what matters is the mapping from input to output. By contrast, the internal state of the widget is irrelevant to the caller.

**Avoid**:
> A widget, which is a data transformer that produces an output from an input, is something in which the internal state is irrelevant to the caller.

### Heading Levels

Headings must be consecutive — never skip a level. If the parent is `*`, the child is `**`, not `***`. A `***` is only valid when preceded by a `**`. This preserves the structural hierarchy for org-mode parsers.

### Key Principles Lists

When a source lists principles, rules, or steps, introduce each as an imperative or directive subheading with a colon, then a short explanatory paragraph:

> *** Maintain idempotency
> Ensure the widget produces the same result when given the same input multiple times. This makes the system predictable and testable.

Each entry is: heading phrase (call to action) + 1-2 sentences of justification or effect. No numbered steps unless the source itself uses them.

### Use of Bold for Key Terms

Introduce key terms in *bold* the first time they appear in a note (org-mode uses single asterisks: *bold*). Use bold sparingly — only for terms that the note itself is defining:

> *Idempotency* is the property of producing the same result on repeated application.

> *Widgets are wholly distinct from filters.*

Do not bold for emphasis. Reserve bold for definitional anchors.
Use org-mode `*bold*`.

### The "Applied to …" Pattern

Optionally end a core note with a concrete analogy from a familiar domain (everyday life, a well-known craft). Use this only when it brings significantly more clarity — an analogy that earns its keep is worth including, but a forced one is worse than none. The analogy is told as a mini-narrative, not stated as a comparison:

> Teaching someone to ride a bicycle illustrates what idempotency means in practice. Each attempt starts from the same place. The outcome — staying upright — is always the same regardless of who tries. The value lies in the predictability of the result, not in the novelty of the attempt.

When used, follow this structure: (1) state the domain, (2) describe a scene or scenario, (3) draw the parallel implicitly — trust the reader to connect it.

### Quote Integration

Quotes from authoritative sources are introduced with a blockquote. Attribution follows on a separate line with an em-dash:

```
#+begin_quote
A widget is only as good as its failure modes.

— Some Authority
#+end_quote
```

Quotes are short (1-3 sentences) and punchy. They are not analysed or explained after being quoted.

### Handling "Why" and "How" Separately

When a concept has both a practical aspect and a theoretical aspect, separate them into distinct notes. The "how" notes cover practical patterns, key principles, and usage. The "why" notes cover foundations, theory, and rationale. Distinction notes cover where the boundary lies between related concepts.

This separation mirrors the source material's own structure — don't invent categories, just keep them cleanly divided.

### Cross-Reference Style

Inline links to other notes into the prose wherever a natural connection exists. The link should sit on the phrase that names the related concept, not tacked on at the end:

> This is closely related to [[id:uuid][widget lifetimes]] but covers a different scope.

> Unlike [[id:uuid][filters]], widgets preserve the full structure of the input.

Do not place links at the end of paragraphs or sections as an afterthought. Avoid separate "See also" sections — if a link belongs, it belongs in the sentence that mentions the concept.

The only exception is hub notes. A hub note serves as an index for a topic area and needs a flat list of links enumerating the constituent notes. That list is the hub's purpose, not an afterthought.

Bidirectional linking (note A links to note B and note B links back to note A) is usually a sign of muddled structure. One direction is almost always enough — choose the direction that serves the reader's flow. Reserve bidirectional links for the rare case where each note genuinely needs the other for context on its own terms, and not merely because a mechanical symmetry feels tidy.

### Inline Verbatim and Code

Use ~ for short non-code technical terms, concepts, notation, or any verbatim emphasis that is not source code: ~O(n)~, ~rpm~, ~/var/log~, ~:ID:~.

Use = for short inline source code fragments only — function names, variables, arguments, expressions: =function_call(2)=, =std::vector<int>=.

Use `#+begin_src` blocks for multi-line code or examples longer than a few words. Specify the language for syntax highlighting:

```
#+begin_src python
def greet(name):
    print(f"hello, {name}")
#+end_src
```

Shell commands and one-liners go in `#+begin_src sh` or `#+begin_src shell`. Keep examples focused — one block per distinct concept, with a short explanatory heading above it.

### Table Style

When converting HTML tables from source material or creating the new tables, keep them as org-mode tables. Use a header row, a separator row, then data rows. Column headers are short noun phrases. 

```
|                    | DPDK                                   | XDP/AF_XDP                               |
|--------------------+----------------------------------------+------------------------------------------|
| Kernel involvement | None — userspace driver                | BPF program in kernel driver             |
| Data path          | Polling, no syscalls                   | Poll or interrupt, descriptor passing    |
| NIC ownership      | Removed from kernel                    | Shared with kernel                       |
| Use case           | Maximum throughput, dedicated hardware | Flexible processing, kernel co-existence |
```

Keep tables narrow — no more than 4-5 columns. If the source table is wider, split it or convert to prose.

### Overall Voice Profile

| Dimension | Choice |
|-----------|--------|
| Register | Neutral, authoritative, plain |
| Person | Third-person (except for direct quotes) |
| Tenses | Present simple (timeless claims) |
| Sentence length | Short to medium; no run-ons |
| Rhetorical mode | Exposition — define, describe, contrast |
| Flavour | Understated; no hyperbole, no marketing language |

The voice is that of a knowledgeable practitioner explaining to a peer. Not a teacher simplifying for a beginner, not an academic building a theory — someone who *does* the thing explaining how it works to someone else who might also do it.

### What to Avoid

- Avoid rhetorical questions ("So what does this mean?")
- Avoid first-person editorialising ("I think", "in my opinion")
- Avoid humour, metaphors, or idioms that depend on cultural knowledge
- Avoid bullet points where prose works — use lists only for enumerations of parallel items
- Avoid empty transitional phrases ("It is important to note that", "It should be noted that")
- Avoid "It is not X — it is Y" constructions. State directly what something is. If a contrast is needed, state both sides plainly without the "not X" framing.

## Workflow

1. **Fetch source material** — retrieve all pages/sections of the material you are capturing.
2. **Create notes** — for each distinct page, run `pkms new "prefix Title" --create` to generate UUIDs and boilerplate files.
3. **Resolve UUIDs** — use `pkms resolve --title "prefix" --output-format json` to get all UUIDs.
4. **Write content** — fill each note following the patterns in the style reference. Inline links to other notes using `[[id:<uuid>][title]]`.
5. **Connect with pkms-manager** — use `pkms resolve` and `pkms query` from the pkms-manager skill to find existing notes for cross-linking.
6. **Verify** — run `pkms check` to confirm no broken links, duplicate UUIDs, or missing titles.
