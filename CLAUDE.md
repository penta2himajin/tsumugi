# Tsumugi

Hierarchical narrative context middleware for creative AI agents. Common core for Tsukasa (TRPG GM aid) and Tsuzuri (novel writing aid). Shares design philosophy with Chatstream but targets text-based creative domains.

## Current Phase

Phase 1 complete (2026-04-23). All 9 core traits defined; Tier 0–1 implementations land with tests. Network-backed providers (`LmStudioEmbedding`, `OpenAiCompatibleProvider`) ship as trait-surface stubs; HTTP wiring is Phase 2. See `docs/concept.md`, `docs/tech-architecture.md`, `docs/TODO.md`.

## Tech Stack

- Implementation: Rust + TypeScript (skeleton generated via `oxidtr`)
- Domain model: `oxidtr` (Alloy → Rust / TypeScript types, tests, invariants)
- Embedding: trait-abstracted (external embedding API / local model)
- Storage: trait-abstracted (initial in-memory impl)
- LLM provider: trait-abstracted (LM Studio / Ollama / cloud APIs)

## Workspace Structure

```
tsumugi/
├── tsumugi-core/        # Core library — domain, traits, context compiler
├── tsumugi-cli/          # Development / verification REPL
├── tsumugi-ts/           # TypeScript SDK (later)
└── models/               # Alloy source of truth (oxidtr input)
```

## Design Principles

- All conversation / passage data is retained. Hierarchy is an index, not a compression artifact.
- Creative-first design: novel chapters, TRPG scenes, game scripts are first-class.
- Library-first: core is a crate. Server / Tauri adapters are optional downstream layers.
- Storage, Embedding, LLM Provider are abstracted as traits; implementations are swappable.
- Deterministic codegen where possible: skeleton from Alloy via oxidtr, business logic written by hand.

## Development Workflow

- Commit after verified step (all tests pass, no warnings).

### TDD (Red-Green-Refactoring)

Every feature / fix follows the cycle:

1. **Red**: write a failing test first.
2. **Green**: write the minimum code that passes.
3. **Refactor**: improve while staying green.

No implementation commit without a test.

## Prohibitions

1. **No deleting / skipping / commenting out existing tests.**
2. **No unauthorized CI config changes** without explicit user instruction.
3. **No degrading production code to make tests pass.**

## Commands

```bash
cargo <cmd>              # Rust
bun run <script>         # Node / Bun
npx <cmd>                # npx
```

## Notes for Claude sessions

- Alloy models under `models/` are the source of truth. Do not edit generated Rust / TS types directly; regenerate via oxidtr.
- `ChunkId`, `TurnId`, `CharacterId`, etc. are newtype-wrapped IDs; do not bypass.
- Async traits use `async-trait` or Rust 1.75+ RPITIT.
- In-memory implementations are used in tests for external-dependency-free fast iteration.
