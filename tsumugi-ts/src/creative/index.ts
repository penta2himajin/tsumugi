// Creative-domain types (feature-analogue of the `creative` Rust feature flag).
// Imported via `tsumugi/creative` subpath so consumers who don't need them
// pay no type-graph or bundle cost.

import type { CharacterId, ChunkId, LoreEntryId } from '../domain/ids.js';

export type Formality = 'Casual' | 'Neutral' | 'Formal';

export interface SpeechTraits {
  readonly formality: Formality;
  readonly quirks: readonly string[];
}

export interface Character {
  readonly id: CharacterId;
  readonly name: string;
  readonly voice_samples: readonly string[];
  readonly speech_traits: SpeechTraits | null;
  readonly relationship_notes: Readonly<Record<string, string>>;
  readonly sheet: Readonly<Record<string, unknown>>;
  readonly first_appearance: ChunkId | null;
  readonly style_tags: readonly string[];
}

export interface SceneView {
  readonly chunk: ChunkId;
  readonly participants: readonly CharacterId[];
  readonly location: string | null;
  readonly time_marker: string | null;
}

export type PoV = 'First' | 'Second' | 'Third';
export type Tense = 'Present' | 'Past';

export interface StylePreset {
  readonly pov: PoV;
  readonly tense: Tense;
  readonly formality: Formality;
  readonly reference_samples: readonly string[];
}

export type LoreScope =
  | { readonly kind: 'Global' }
  | { readonly kind: 'ChunkLocal'; readonly chunk: ChunkId }
  // Mirrors the Rust `ConditionalScope` non-empty-string newtype — the Rust
  // side rejects empty/whitespace-only values, so the TS side treats this as
  // a brand to nudge consumers through a constructor.
  | { readonly kind: 'Conditional'; readonly expr: ConditionalExpr };

declare const ConditionalBrand: unique symbol;
export type ConditionalExpr = string & { readonly [ConditionalBrand]: 'ConditionalExpr' };

export const LoreScope = {
  global(): LoreScope {
    return { kind: 'Global' };
  },
  chunkLocal(chunk: ChunkId): LoreScope {
    return { kind: 'ChunkLocal', chunk };
  },
  conditional(expr: string): LoreScope {
    if (expr.trim() === '') {
      throw new TypeError('LoreScope.conditional: expression must be non-empty');
    }
    return { kind: 'Conditional', expr: expr as ConditionalExpr };
  },
};

export interface LoreEntry {
  readonly id: LoreEntryId;
  readonly category: string;
  readonly title: string;
  readonly content: string;
  readonly scope: LoreScope;
  readonly keywords: readonly string[];
}
