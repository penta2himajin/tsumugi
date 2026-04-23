import { describe, expect, it, vi } from 'vitest';

import { ChunkId, FactId, FactScope } from '../src/index.js';
import type { Chunk, Fact } from '../src/index.js';
import { createTauriClient, TSUMUGI_COMMANDS } from '../src/tauri/index.js';
import type { CompiledContext, InvokeFn } from '../src/tauri/index.js';

const UUID_A = '11111111-2222-3333-4444-555555555555';
const UUID_B = '11111111-2222-3333-4444-555555555556';

function fakeChunk(id = UUID_A): Chunk {
  return {
    id: ChunkId.from(id),
    text: 'hello',
    items: [],
    summary: '',
    keywords: [],
    facts: [],
    pending: [],
    parent: null,
    children: [],
    metadata: {},
    last_active_at: new Date().toISOString(),
    order_in_parent: 0,
    source_location: null,
    summary_level: 0,
    summary_method: 'None',
    edited_by_user: false,
    auto_update_locked: false,
  };
}

describe('createTauriClient', () => {
  it('saveChunk forwards to the canonical command name', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(undefined);
    const client = createTauriClient(invoke);
    const chunk = fakeChunk();
    await client.saveChunk(chunk);
    expect(invoke).toHaveBeenCalledWith(TSUMUGI_COMMANDS.saveChunk, { chunk });
  });

  it('loadChunk passes id and returns the parsed response', async () => {
    const chunk = fakeChunk();
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(chunk);
    const client = createTauriClient(invoke);
    const got = await client.loadChunk(ChunkId.from(UUID_A));
    expect(invoke).toHaveBeenCalledWith(TSUMUGI_COMMANDS.loadChunk, {
      id: UUID_A,
    });
    expect(got).toBe(chunk);
  });

  it('listChunks returns the backend response', async () => {
    const ids = [ChunkId.from(UUID_A), ChunkId.from(UUID_B)];
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(ids);
    const client = createTauriClient(invoke);
    const got = await client.listChunks();
    expect(invoke).toHaveBeenCalledWith(TSUMUGI_COMMANDS.listChunks);
    expect(got).toEqual(ids);
  });

  it('saveFact round-trips the fact payload', async () => {
    const fact: Fact = {
      id: FactId.from(UUID_A),
      key: 'hp',
      value: '12',
      scope: FactScope.global(),
      superseded_by: null,
      created_at: new Date().toISOString(),
      origin: 'User',
    };
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(undefined);
    const client = createTauriClient(invoke);
    await client.saveFact(fact);
    expect(invoke).toHaveBeenCalledWith(TSUMUGI_COMMANDS.saveFact, { fact });
  });

  it('compileContext sends the request body under `request`', async () => {
    const response: CompiledContext = {
      query: 'alice',
      resident_chunks: [fakeChunk()],
      active_facts: [],
      dynamic_chunks: [],
    };
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(response);
    const client = createTauriClient(invoke);
    const got = await client.compileContext({ query: 'alice', current_chunk_id: null });
    expect(invoke).toHaveBeenCalledWith(TSUMUGI_COMMANDS.compileContext, {
      request: { query: 'alice', current_chunk_id: null },
    });
    expect(got).toBe(response);
  });

  it('propagates invoke errors verbatim', async () => {
    const invoke = vi
      .fn<InvokeFn>()
      .mockRejectedValue(new Error('tauri: command not registered'));
    const client = createTauriClient(invoke);
    await expect(client.loadChunk(ChunkId.from(UUID_A))).rejects.toThrow(
      'tauri: command not registered',
    );
  });
});
