// tsumugi — TypeScript SDK (primary target: Tauri frontends, also usable
// in Node / Bun server runtimes).
//
// The `./` entry point exposes the runtime-facing domain surface only so a
// consumer who doesn't need creative / Tauri bits stays free of that type
// graph. Subpaths:
//
//   tsumugi            → runtime domain types (Chunk / Fact / PendingItem /
//                         SourceLocation / Ids / SummaryMethod)
//   tsumugi/creative   → creative extension (Character / LoreEntry / ...)
//   tsumugi/tauri      → Tauri IPC client helpers
//   tsumugi/gen        → oxidtr-generated types (Alloy relational skeleton)

export const version = '0.0.0' as const;

export * from './domain/index.js';
