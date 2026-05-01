# Tsumugi

## Overview

Tsumugi is a general-purpose memory-layer framework for LLM
applications. The core engine treats conversations and passages as a
hierarchy of `Chunk`s with summarisation, query classification, and
prompt compression abstractions, and exposes only domain-agnostic
primitives. Domain-specific extensions are expected to live in
downstream consumers, not in tsumugi itself.

The `docs/` directory is the source of truth for design and
specification (`concept.md`, `tech-architecture.md`,
`context-management-survey.md`, `TODO.md`). This file only covers the
workflow contracts that are not in `docs/`.

## Project Structure

```
tsumugi-core/   # Core library — domain model, traits, context compiler
tsumugi-cli/    # Development / verification REPL
tsumugi-ts/     # TypeScript SDK (oxidtr-generated types + runtime)
models/         # Alloy source of truth (oxidtr input)
docs/           # Design specifications and research surveys
scripts/        # Codegen / drift-check helpers
```

## Development Setup

Toolchain pins live in `rust-toolchain.toml` (Rust) and `package.json`
(TypeScript / Bun). No additional bootstrap is required beyond:

```bash
rustup show                  # honours rust-toolchain.toml
bun install                  # tsumugi-ts dependencies
```

Alloy and `oxidtr` are required for type regeneration. `oxidtr` is
expected as a sibling clone (`../oxidtr`) or supplied via
`OXIDTR_HOME=…`.

## Build & Test

```bash
cargo build --workspace
cargo test  --workspace
cargo test  --workspace --all-features
bun run --cwd tsumugi-ts typecheck
bun run --cwd tsumugi-ts test
```

CI runs the Alloy drift check across both Rust and TypeScript
generated subtrees; regenerate locally before committing changes that
touch `models/`.

## Development Principle: TDD (Red → Green → Refactor)

All implementation work proceeds in this cycle:

1. **Red**: write a failing test that captures the intended behaviour;
   confirm it fails for the right reason with `cargo test`.
2. **Green**: write the minimum code that makes the test pass.
3. **Refactor**: tidy up while keeping tests green.

No implementation commit without an accompanying test. Follow the
phase order in [`docs/TODO.md`](./docs/TODO.md); do not start a phase
before the previous one's done-criteria are met.

## Architectural Boundaries

- **Core stays domain-agnostic.** `tsumugi-core` exposes a
  general-purpose memory-layer API. Domain-specific types belong in
  downstream consumers and must not be added to tsumugi itself.
- **Full history is retained; injection is selective.** All
  conversation / passage data stays in storage. LLM inputs are
  compiled selectively from the hierarchy — never a full-history
  dump.
- **Tiered processing escalates only when needed.** Prefer
  Tier 0 (deterministic / BM25) → Tier 1 (small encoders / classifiers)
  → Tier 2 (encoder-only ONNX impls: LlmLingua-2, NLI zero-shot,
  DistilBART). tsumugi removed all autoregressive LLM calls in
  2026-04; do not reintroduce an `LLMProvider` trait or LLM-backed
  impls. Downstream consumers that need an LLM call bridge to one
  outside tsumugi-core, against the `CompiledContext` it produces.
- **Storage and embedding are trait-abstracted.** `tsumugi-core` does
  not depend on a concrete vector DB or embedding API. In-memory
  / mock implementations are the test default; `OnnxEmbedding` is the
  production embedding path under the `onnx` feature.
- **Newtype IDs are not bypassed.** `ChunkId`, `FactId`,
  `PendingItemId` are wrapper types; do not pass raw strings or
  integers through them.
- **Alloy models are canonical.** `models/` drives generated Rust and
  TypeScript types via oxidtr. Do not hand-edit files under any
  `gen/` directory.

## Prohibitions

1. **Do not delete or disable existing tests.** If a test fails, fix
   the production code, not the test. Skipping or commenting out
   tests is not acceptable.
2. **Do not commit credentials or provider API keys.** Local LLM
   endpoints are fine; cloud provider keys, embedding API keys, and
   anything in `.env*` files stay out of the repository.
3. **Do not modify CI configuration without explicit instruction.**
   Files under `.github/workflows/` are not changed without the user
   asking for it.
4. **Do not bypass the phase order.** `docs/TODO.md` defines the
   build sequence and done-criteria. Do not write later-phase code
   before the current phase is verified, even if it looks ready.
5. **Do not add domain-specific types to `tsumugi-core`.** The core
   stays a generic memory-layer framework; product-specific
   abstractions live in their own crates / repositories.
6. **Do not edit oxidtr-generated files directly.** Regenerate via
   the appropriate script after editing the relevant Alloy model.

## Git Conventions

- **Conventional Commits**: `feat:`, `fix:`, `docs:`, `refactor:`,
  `test:`, `ci:`, `chore:`.
- **Branch naming**: `claude/<topic>` for Claude-driven work.
- **Tests must pass and warnings must be zero before committing.**
- **Append a `Co-Authored-By` trailer** to commits Claude authors,
  for transparency:

  ```
  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

- Documentation-only changes go in `docs:` commits; mixed code +
  documentation changes are split into separate commits where it does
  not break atomicity.

## Claude Session Guidance

Claude Code cloud sessions occasionally fail with `Stream idle
timeout` on long output. To reduce the risk:

1. **Stage long writes.** For long documents or source files, write
   the skeleton (headings, function signatures, trait stubs) first,
   then fill each section in follow-up edits. Avoid single blocks
   larger than ~200 lines.
2. **Watch out after large reads.** Reading a big file (e.g.
   `Cargo.lock`, large generated modules) and then immediately
   producing long output is a common trigger. Split into separate
   turns or excerpt only the relevant portion.
3. **Recover carefully.** A timeout can still leave the file write
   completed. Run `git status` before retrying so the same content is
   not written twice.
