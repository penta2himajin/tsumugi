// SourceLocation — B 案 (2026-04-23 確定): sum type with rich payloads.
//
// Mirrors the Rust `SourceLocationValue` enum with `#[serde(tag = "kind",
// rename_all = "kebab-case")]`. When the Rust side serializes a
// `SourceLocationValue::File(FileSourceLocation)` it produces:
//   { "kind": "file", "path": "...", "span": null | [start, end] }
// and `Custom { schema, payload }` becomes:
//   { "kind": "custom", "schema": "...", "payload": ... }

export interface Span {
  readonly start: number;
  readonly end: number;
}

export interface FileSourceLocation {
  readonly path: string;
  readonly span?: Span | null;
}

export type SourceLocationValue =
  | ({ readonly kind: 'file' } & FileSourceLocation)
  | {
      readonly kind: 'custom';
      readonly schema: string;
      // Opaque product-supplied payload. Typed as unknown to force callers
      // through a product-specific type guard before use.
      readonly payload: unknown;
    };

/**
 * Behavior-facing view. Mirrors `trait SourceLocation` on the Rust side.
 * `schema()` and `path()` are derived from the storage form so callers don't
 * need to branch on the variant for the common cases.
 */
export interface SourceLocation {
  schema: string;
  path: string;
  span?: Span | null;
  proximity(other: SourceLocation): number;
}

export const SourceLocationValue = {
  file(path: string, span?: Span | null): SourceLocationValue {
    return { kind: 'file', path, span: span ?? null };
  },
  custom(schema: string, payload: unknown): SourceLocationValue {
    return { kind: 'custom', schema, payload };
  },
};

/** Same-path prefix count. Matches the Rust `file_proximity` shape. */
function fileProximity(a: string, b: string): number {
  if (a === b) return 1.0;
  const aParts = a.split('/').filter(Boolean);
  const bParts = b.split('/').filter(Boolean);
  let common = 0;
  for (let i = 0; i < Math.min(aParts.length, bParts.length); i++) {
    if (aParts[i] === bParts[i]) common++;
    else break;
  }
  const maxDepth = Math.max(aParts.length, bParts.length, 1);
  return common / maxDepth;
}

export function asSourceLocation(value: SourceLocationValue): SourceLocation {
  if (value.kind === 'file') {
    return {
      schema: 'file',
      path: value.path,
      span: value.span ?? null,
      proximity(other: SourceLocation): number {
        if (other.schema !== 'file') return 0;
        return fileProximity(value.path, other.path);
      },
    };
  }
  return {
    schema: value.schema,
    path: value.schema,
    span: null,
    proximity(other: SourceLocation): number {
      return other.schema === value.schema ? 1 : 0;
    },
  };
}
