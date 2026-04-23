// Tauri IPC helper layer.
//
// tsumugi-ts does not depend on `@tauri-apps/api` directly so consumers can
// pick their own Tauri version (v1 vs v2 have different import paths and
// coexistence in the same tree is painful). Instead, consumers pass the
// platform's `invoke` function in, and we build a typed client around it.
//
// Usage example (Tauri v2):
//
//   import { invoke } from '@tauri-apps/api/core';
//   import { createTauriClient } from 'tsumugi/tauri';
//   const tsumugi = createTauriClient(invoke);
//   await tsumugi.saveChunk(myChunk);
//
// On the Rust side, the product app (つかさ / つづり / つくも) registers
// matching `#[tauri::command]` handlers whose names are the `TSUMUGI_COMMANDS`
// constants below. This keeps the command namespace stable across clients.

import type { Chunk } from '../domain/chunk.js';
import type { Fact } from '../domain/fact.js';
import type { ChunkId, FactId, PendingItemId } from '../domain/ids.js';
import type { PendingItem } from '../domain/pending.js';

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/**
 * Canonical command names. The Rust app registers these as `#[tauri::command]`
 * handlers. Keeping them in a single constant avoids typos creeping into the
 * client / server on either side.
 */
export const TSUMUGI_COMMANDS = {
  saveChunk: 'tsumugi_save_chunk',
  loadChunk: 'tsumugi_load_chunk',
  deleteChunk: 'tsumugi_delete_chunk',
  listChunks: 'tsumugi_list_chunks',
  saveFact: 'tsumugi_save_fact',
  loadFact: 'tsumugi_load_fact',
  deleteFact: 'tsumugi_delete_fact',
  listFacts: 'tsumugi_list_facts',
  savePending: 'tsumugi_save_pending',
  loadPending: 'tsumugi_load_pending',
  deletePending: 'tsumugi_delete_pending',
  listPending: 'tsumugi_list_pending',
  compileContext: 'tsumugi_compile_context',
} as const;

export interface CompileRequest {
  readonly query: string;
  readonly current_chunk_id?: ChunkId | null;
}

export interface CompiledContext {
  readonly query: string;
  readonly resident_chunks: readonly Chunk[];
  readonly active_facts: readonly Fact[];
  readonly dynamic_chunks: readonly { readonly chunk: Chunk; readonly score: number }[];
}

export interface TsumugiClient {
  saveChunk(chunk: Chunk): Promise<void>;
  loadChunk(id: ChunkId): Promise<Chunk>;
  deleteChunk(id: ChunkId): Promise<void>;
  listChunks(): Promise<readonly ChunkId[]>;

  saveFact(fact: Fact): Promise<void>;
  loadFact(id: FactId): Promise<Fact>;
  deleteFact(id: FactId): Promise<void>;
  listFacts(): Promise<readonly FactId[]>;

  savePending(item: PendingItem): Promise<void>;
  loadPending(id: PendingItemId): Promise<PendingItem>;
  deletePending(id: PendingItemId): Promise<void>;
  listPending(): Promise<readonly PendingItemId[]>;

  compileContext(request: CompileRequest): Promise<CompiledContext>;
}

/**
 * Build a typed Tauri client on top of the given invoke function.
 *
 * All methods simply forward to `invoke` with the command name from
 * `TSUMUGI_COMMANDS` and the argument wrapped in a record matching the Rust
 * `#[tauri::command]` signature convention.
 */
export function createTauriClient(invoke: InvokeFn): TsumugiClient {
  return {
    async saveChunk(chunk) {
      await invoke(TSUMUGI_COMMANDS.saveChunk, { chunk });
    },
    async loadChunk(id) {
      return (await invoke(TSUMUGI_COMMANDS.loadChunk, { id })) as Chunk;
    },
    async deleteChunk(id) {
      await invoke(TSUMUGI_COMMANDS.deleteChunk, { id });
    },
    async listChunks() {
      return (await invoke(TSUMUGI_COMMANDS.listChunks)) as readonly ChunkId[];
    },

    async saveFact(fact) {
      await invoke(TSUMUGI_COMMANDS.saveFact, { fact });
    },
    async loadFact(id) {
      return (await invoke(TSUMUGI_COMMANDS.loadFact, { id })) as Fact;
    },
    async deleteFact(id) {
      await invoke(TSUMUGI_COMMANDS.deleteFact, { id });
    },
    async listFacts() {
      return (await invoke(TSUMUGI_COMMANDS.listFacts)) as readonly FactId[];
    },

    async savePending(item) {
      await invoke(TSUMUGI_COMMANDS.savePending, { item });
    },
    async loadPending(id) {
      return (await invoke(TSUMUGI_COMMANDS.loadPending, { id })) as PendingItem;
    },
    async deletePending(id) {
      await invoke(TSUMUGI_COMMANDS.deletePending, { id });
    },
    async listPending() {
      return (await invoke(TSUMUGI_COMMANDS.listPending)) as readonly PendingItemId[];
    },

    async compileContext(request) {
      return (await invoke(TSUMUGI_COMMANDS.compileContext, {
        request,
      })) as CompiledContext;
    },
  };
}
