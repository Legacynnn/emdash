# 0015: `search/` subsystem — port to v1 (L; depends on EMD-11)

## Status

Accepted

## Context

`src/main/core/search/` (~578 lines) powers the command palette's
fuzzy search across project files, with workspace-aware indexing
that tracks file additions/removals/renames via the fs watcher.
Heavy: index storage, fuzzy ranking, query parser, file-watch
debounce.

## Decision

**Port to v1 (L).** The command palette is one of the most-used
surfaces in the Electron build; shipping v1 without it would be a
material regression. **Depends on EMD-11** (file watching + MCP)
because the index needs `notify` watchers to stay fresh.

**Scope (large) — separate Linear issue:**
- `src/search/` domain module:
  - `WorkspaceFileIndex` (in-memory trie or sorted vec — TBD by
    benchmarking on a 100k-file repo)
  - Fuzzy ranker (`fuzzy-matcher` crate is the obvious choice)
  - Query parser (mirrors the Electron palette's modifiers:
    `>command`, `:file`, etc.)
- `notify`-driven invalidation (adds/removes/renames update the
  index in real time — depends on EMD-11)
- `search_command_palette(query) -> Vec<SearchResult>` command +
  specta + insta snapshot
- Renderer command-palette UI (Mac-style overlay) — separate UX
  pass

## Follow-up

File Linear issue **EMD-XX: Port search subsystem (L, blocked on
EMD-11)**.

## Consequences

- Command-palette search is unavailable until both EMD-11 and this
  follow-up land. v1's renderer ships without the omni-search
  surface — the user has to navigate projects/tasks via the side
  panel until the palette lands.
- We commit to the `fuzzy-matcher` crate (or a benchmarked
  alternative). If it doesn't keep up on a 100k-file repo, we
  re-evaluate at port time.
