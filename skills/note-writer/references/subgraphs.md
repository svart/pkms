# Subgraph Reference

Use this reference when connecting an isolated cluster of notes to the rest of the graph.

## Shared Technology Bridges

Connect isolated clusters through concepts they genuinely share with the main graph. List the technologies, standards, measurements, or principles in the isolated cluster, then check which already exist elsewhere.

Example pattern:

```text
Wi-Fi cluster: OFDM, MIMO, RSSI, signal-to-noise ratio, QAM modulation.
Main graph: LTE OFDM, LTE MIMO, RSSI, SINR.
Bridge candidates: OFDM, signal quality, MIMO.
```

A bridge note defines the shared concept in a general way and connects technology-specific notes on both sides.

## Prefer Existing Intermediate Nodes

Before creating a bridge note, check whether an existing note already sits on the boundary. Use that note as the attachment point when it gives a better path than a direct hub link.

Example:

```text
The mobile identity cluster needs a path toward LTE.
The 4g note already sits between 3G and LTE.
Use UMTS -> 4g -> LTE rather than UMTS -> LTE.
```

This keeps hubs from becoming overloaded.

## Merge Duplicates First

If the cluster contains duplicate notes for the same concept, merge before linking outward.

Merge by:

1. Keeping the note with the strongest content or most useful inbound links.
2. Consolidating content into the kept note.
3. Updating links that pointed to the discarded UUID.
4. Removing the duplicate file only when it is no longer referenced.

Do not create bridges around duplicate concepts.

## Choose Navigable Path Lengths

Prefer short but meaningful paths through intermediate concepts instead of direct hub links. A direct hub link is useful only when the note is truly a direct child of the hub.

Examples:

```text
SINR -> CQI -> LTE
RSRP -> SINR -> CQI -> LTE
802.11ax -> OFDM -> LTE OFDM -> LTE
```

Use `pkms path <from> <to>` to inspect path length and graph shape. More hops can be acceptable when each hop represents a real conceptual relationship.

## Fill Stubs Before Bridging

An empty note does not make a useful bridge. Before connecting a cluster outward, give each bridge or endpoint enough content to reward reading:

- A definition sentence.
- A boundary sentence.
- At least one justified internal link when a real target exists.

## Build Independent Bridges Sparingly

A cluster should not depend on a single fragile bridge when several real shared concepts exist. Build multiple independent bridge paths only when each path has its own justification.

Example:

```text
wifi -> wireless signal quality -> RSSI -> SINR -> LTE
802.11ax -> OFDM -> LTE OFDM -> LTE
802.11ac -> MIMO -> LTE MIMO -> LTE
```

Do not add extra bridges only to increase link count.

## Link Direction

Choose link direction for reader flow:

- Specific notes usually link to general concepts.
- Technology-specific variants usually link to shared abstractions.
- Evolutionary chains can link from newer concepts to older context when that is the natural explanation path.
- Bridge notes may link outward when the bridge itself is a useful place to compare both sides.

Backlinks can carry the reverse discovery path when an explicit reverse link would be redundant.
