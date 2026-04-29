// Branded string newtypes for the domain IDs.
//
// These are `string` at runtime (UUID in canonical hyphenated form) but branded
// at the type level so a `ChunkId` cannot be accidentally passed where a
// `FactId` is expected.

declare const TsumugiBrand: unique symbol;

type Brand<T, B> = T & { readonly [TsumugiBrand]: B };

export type ChunkId = Brand<string, 'ChunkId'>;
export type FactId = Brand<string, 'FactId'>;
export type PendingItemId = Brand<string, 'PendingItemId'>;

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function isUuid(value: string): boolean {
  return UUID_REGEX.test(value);
}

/**
 * Construct an ID from a string. Validates the UUID shape and throws on bad
 * input so callers can't smuggle malformed values through.
 */
function makeId<T extends string>(name: string, value: string): T {
  if (!isUuid(value)) {
    throw new TypeError(`${name} must be a UUID (got: ${JSON.stringify(value)})`);
  }
  return value as T;
}

export const ChunkId = {
  from: (value: string): ChunkId => makeId('ChunkId', value),
};
export const FactId = {
  from: (value: string): FactId => makeId('FactId', value),
};
export const PendingItemId = {
  from: (value: string): PendingItemId => makeId('PendingItemId', value),
};
