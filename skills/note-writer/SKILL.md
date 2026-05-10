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

### Heading-Level `:ID:` Properties

When a heading covers a concept that deserves its own link anchor, add a PROPERTIES drawer with `:ID:` immediately after the heading line:

```org
** HTTP/2
:PROPERTIES:
:ID:       bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb
:END:
```

Generate heading UUIDs with `pkms new "Note Title" --create --heading "HTTP/2"`.
Link to a heading from another note with:

```
[[id:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb][HTTP/2]]
```

The link description should match the context of the surrounding text.

Heading IDs are warranted when:
- The heading covers a sub-concept that other notes need to reference directly.
- The heading is a version/variant of the main topic (e.g., HTTP/2 under HTTP).
- The heading is a distinct protocol, specification, standard, or case within a broader note.

Do not add heading IDs mechanically to every heading. Only add them when cross-note linking to that specific section is expected.

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

### Link Justification

A link must be justified by the immediate sentence, not by general relevance. A link belongs only when the linked concept is the *direct object* of what the sentence is literally about — the sentence would be incomplete or inaccurate without it.

- Don't add links to hub notes from child notes. A child note about a specific feature should link only to its parent topic note, not to broadly related hubs.
- Don't enumerate platforms or ecosystems on general concept notes (html, markdown, etc.). A concept note defines the concept; it does not list where it is used.
- "Comparable to" links are appropriate in a main topic note (e.g. gitlab → github), but not in child notes or concept notes.
- Don't create intermediate notes to rescue a broken link. Remove the broken link instead if you cannot find corresponding note in the database.
- Prefer the most general note when multiple candidates match. Link "DNS" to the `dns` note, not `dns record types`. Check aliases — a concept may be recorded under a variant name. Inspect candidates with `pkms resolve --title "term"` and `pkms get <target> --no-content`.
- A link must earn its place by adding genuine navigation value. Remove links when: the referenced concept is already the subject of the current note (self-referencing), the description is a filename or proper noun with no corresponding note, or the sentence already provides the needed context through another mechanism (a direct URL, a code example, or the definition itself).
- Prefer external URLs over internal links for reference and background context. An Arch Wiki link belongs as an external URL, not as a link to an internal Arch Linux note.
- One link per sentence is enough. Multiple links in a single sentence dilute each link's justification.

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

## Linking Isolated Subgraphs

When you discover a cluster of notes disconnected from the main graph, connect
it through shared concepts rather than forcing direct links to an overloaded
hub. These patterns emerge from practical experience linking three disconnected
subgraphs (cellular signal metrics, Wi-Fi, mobile network identities) into a
well-connected wireless networking graph.

### Identify Shared Technology Bridges

Isolated subgraphs are usually focused on a specific domain. Find the concepts
it shares with the main graph — those are your bridge points. List the
technologies, standards, or measurements the subgraph covers, then check which
of those already exist in the main graph.

> Wi-Fi subgraph covers: OFDM, MIMO, RSSI, signal-to-noise ratio, QAM modulation.
> Main graph already has: lte ofdm, lte mimo, RSSI, SINR.
> → Bridges: OFDM (shared PHY technique), signal quality (shared metric).

Each shared concept becomes a bridge. Create one bridging note per concept. A
bridging note defines the concept in a general, cross-technology way and links
to the technology-specific notes on both sides.

### Prefer Existing Intermediate Nodes over Direct Links

Before creating a new bridging note, check whether an existing note in the main
graph already sits on the boundary. If a note like "4g" or "CQI" already
connects to the main hub (LTE), use it as the attachment point instead of
linking directly to the hub. This keeps the hub from becoming overlinked.

> MCC subgraph needs connection to LTE. The "4g" note already sits between 3G and LTE.
> → Link UMTS → 4g (not UMTS → LTE). Path becomes UMTS → 4g → LTE (2 hops).

When no suitable intermediate exists, create a bridging note. A bridging note
earns its place by defining a concept that genuinely spans both sides — RSSI and
SNR are the same physical measurements in Wi-Fi and LTE; OFDM is the same
modulation technique.

### Merge Duplicates before Connecting

If the subgraph contains two notes about the same concept under different
titles, merge them into one note before connecting outward. Duplicates create
split attention and force readers to guess which note to read.

> "wifi" (empty) and "wi-fi" (one sentence) are the same concept.
> → Merge into one "wifi" note with content. Redirect links from the discarded UUID.

Merge by: 
1. picking the note with the most inbound links to keep; 
2. consolidating content into it;
3. updating all links that point to the discarded UUID;
4. deleting the discarded file.

### Prefer 2-3 Hops from the Hub

A hub note with too many incoming links becomes hard to navigate (the user
mentioned LTE with 13 backlinks). When connecting a subgraph, aim for paths of
2-3 hops from the hub, not direct links. Each hop distributes the cognitive load
across intermediate notes.
Sometimes more hops are necessary for better linking logic. Tell user about it.

> Instead of linking signal metrics directly to LTE:
> SINR → CQI → LTE (2 hops).
> RSRP → SINR → CQI → LTE (3 hops).

Use `pkms path <from> <to>` to check the hop count before committing to a linking strategy.

### Fill Stub Notes with Minimal Viable Content

An empty note with only a title is a dead end. Before linking a subgraph
outward, give each stub note enough content to be useful on its own. Follow a
minimal template:

- A definition sentence (what it is, in context)
- A boundary sentence (what distinguishes it from related concepts)
- At least one inline link to another note in the same subgraph

This ensures the note rewards reading and provides a base for future expansion.
The `pkms validate` output flags notes with 0 outgoing links — prefer each note
to have at least one internal link.

### Build Several Independent Bridge Paths

A subgraph connected through a single bridge is fragile — if that bridge note is
ever restructured, the subgraph becomes orphaned. Create 2-3 independent bridge
paths through different shared concepts. 

> Wi-Fi subgraph connects via:
> (a) wifi → wireless signal quality → RSSI → SINR → ... → LTE
> (b) 802.11ax → OFDM → lte ofdm → LTE
> (c) 802.11ac → lte mimo → LTE

Each path stands on its own. If one bridge is removed, the subgraph remains connected through the others.

But don't overuse it. If it is completely enough to one path between notes, then leave it.

### Link Direction

When connecting subgraphs through a bridging note, the link direction should
serve the reader's flow:

- From specific to general: a technology-specific note links to the general bridging concept (802.11ac → OFDM), not the reverse
- From newer to older in an backward evolutionary chain: 4g → UMTS → GSM . So user will follow backlinks if necessary.
- The bridging note may be linked by itself or link outward to specific notes. It depends on the case.

This keeps the graph navigable: a reader starting in the subgraph naturally
discovers the bridge, follows it to the general concept, and from there reaches
the main graph.
