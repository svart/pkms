# Crate: pkms-tokens

`crates/pkms-tokens` is a deliberately small leaf crate for token encoding,
counting, and truncation. It isolates `tiktoken-rs` from org parsing and from
crates that do not use token budgets.

## Responsibilities

- Parse the supported `cl100k_base` and `o200k_base` encoding names.
- Count tokens using the selected encoding.
- Truncate text to a caller-provided token limit.

The crate does not decide which encoding or budget a command should use. Those
are caller policies in `pkms-db`, `pkms-rag`, or the umbrella CLI adapter.

## Boundaries

`pkms-tokens` must not depend on `pkms` or any domain crate. The boundary check
enforces this leaf direction transitively.

## Related Docs

- [Architecture](../architecture.md)
- [RAG Retrieval](../rag.md)
- [Development](../development.md)
