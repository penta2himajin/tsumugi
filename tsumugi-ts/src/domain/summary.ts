// SummaryMethod — matches the Rust enum serde representation
// (`#[serde(rename_all = "...")]` is not set on the Rust side, so variant
// names map 1:1).

export type SummaryMethod =
  | 'LlmFull'
  | 'LlmLingua2'
  | 'SelectiveContext'
  | 'ExtractiveBM25'
  | 'DistilBart'
  | 'UserManual'
  | 'None';

export function isRawLeafMethod(m: SummaryMethod): boolean {
  return m === 'None';
}
