# SetFit query-classifier training

This directory holds the inputs that produce the default trained head
that tsumugi ships with — `models/setfit/all-MiniLM-L6-v2-default.{onnx,tokenizer.json,head.json}`.

## What's here

- **`queries.jsonl`** — 4 labels × 16 examples = 64 lines. The seed
  dataset for SetFit fine-tuning. Each row: `{"text": "...", "label":
  "Literal|Narrative|Analytical|Unknown"}`.
- **`README.md`** (this file) — re-train procedure, label definitions,
  reproducibility notes.

The trained `head.json` and `tokenizer.json` live next door at
`models/setfit/all-MiniLM-L6-v2-default.{head.json,tokenizer.json}`
and are committed in plain Git (small enough to not need LFS).

## ONNX encoder weights

The trained encoder ONNX (~86 MB) is **not** in the initial PR-2
commit because the Claude Code on the web environment's local Git proxy
doesn't support LFS uploads (returns 502 to the LFS batch endpoint).
Run `scripts/train_setfit.py` locally and commit the resulting
`models/setfit/all-MiniLM-L6-v2-default.onnx` from your own clone —
LFS works there because it talks to real GitHub.

Until that ONNX is on `main`, `SetFitClassifier::from_dir_and_stem`
points at a missing file and `classify` will bail with the standard
"failed to load ONNX model from ..." error. Consumers who need the
classifier working immediately can either run the training locally
or substitute their own encoder via
`SetFitClassifier::with_embedder(...)`.

## Label definitions

The four labels match `tsumugi_core::traits::classifier::QueryClass`.
Downstream consumers wanting different labels train their own head and
pass it in via `SetFitClassifier::new(...)`.

- **Literal** — factual lookup, no reasoning required. Examples:
  "What's the capital of France?", "How many bytes in a kilobyte?".
- **Narrative** — continuation of an ongoing exchange / story. Needs
  recent context + character state. Examples: "Continue the story from
  where Alice met Bob.", "What did the user say in the last turn?".
- **Analytical** — analysis / reasoning over content. Often needs
  structured summaries or multi-source synthesis. Examples: "Why did
  this approach fail?", "Compare these two policies.".
- **Unknown** — fallback for inputs that don't fit any of the above.
  Includes empty / whitespace / gibberish queries plus short
  acknowledgements ("ok", "hmm").

## Re-training

```bash
# 1. Edit queries.jsonl — add / remove / relabel rows. Keep at least
#    8 examples per class (SetFit paper's lower bound for stable
#    fine-tuning).
# 2. Install pinned deps (these specific versions are needed because
#    setfit 1.1.0 is incompatible with newer transformers / sentence-
#    transformers / datasets / optimum releases as of late 2026):
pip install \
  setfit==1.1.0 \
  transformers==4.45.2 \
  sentence-transformers==3.2.1 \
  datasets==2.21.0 \
  optimum[onnxruntime]==1.23.3 \
  onnxruntime==1.20.1 \
  torch==2.4.1

# 3. Run training (~1-2 min CPU on a modern laptop):
python3 scripts/train_setfit.py

# 4. Sanity check the resulting head:
python3 -c "import json; d=json.load(open('models/setfit/all-MiniLM-L6-v2-default.head.json')); print(d['labels'], d['embedding_dim'], len(d['weights']), 'x', len(d['weights'][0]))"
# expected: ['Literal', 'Narrative', 'Analytical', 'Unknown'] 384 4 x 384

# 5. Verify against the env-gated Rust smoke test:
TSUMUGI_MINILM_MODEL_PATH=$(pwd)/models/setfit/all-MiniLM-L6-v2-default.onnx \
TSUMUGI_MINILM_TOKENIZER_PATH=$(pwd)/models/setfit/all-MiniLM-L6-v2-default.tokenizer.json \
TSUMUGI_SETFIT_HEAD_PATH=$(pwd)/models/setfit/all-MiniLM-L6-v2-default.head.json \
  cargo test -p tsumugi-core --features onnx classify_real_weights_returns_known_class
```

## Reproducibility

`train_setfit.py` uses a fixed `SEED = 0xC0FFEE` so re-runs with the
same `queries.jsonl` + same dependency versions produce a bit-identical
`head.json`. The encoder ONNX is **not** bit-identical between runs
(Optimum's ONNX export non-determinism), but its weights are; tsumugi-
core only consumes the encoder embedding so the runtime behaviour is
unaffected.

## Held-out sanity check

After training the default head, the following 6 held-out queries
classify as expected (5/6 = 83%, in line with SetFit paper's reported
80-90% range for 16-examples-per-class fine-tuning):

| query | expected | predicted |
|---|---|---|
| How tall is Mount Fuji? | Literal | Literal ✓ |
| Compare GDP across countries | Analytical | Analytical ✓ |
| Continue the story from where the dragon appeared | Narrative | Narrative ✓ |
| um | Unknown | Unknown ✓ |
| What is 2+2? | Literal | Literal ✓ |
| Why did the empire fall? | Analytical | Literal ✗ |

The last miss ("Why did the empire fall?") is a known borderline case
between Literal (factual lookup of historical event) and Analytical
(causal reasoning). Adding more "why" / "how come" examples to the
Analytical bucket in `queries.jsonl` would tighten this boundary.

## Multilingual

The default head pairs with English-only `all-MiniLM-L6-v2`. For
Japanese / multilingual queries, swap the encoder via
`SetFitClassifier::with_embedder(...)`:

```rust
let multi_embedder = OnnxEmbedding::new(
    "/path/to/paraphrase-multilingual-MiniLM-L12-v2.onnx",
    "/path/to/paraphrase-multilingual-MiniLM-L12-v2.tokenizer.json",
    384,
);
let classifier = SetFitClassifier::from_dir_and_stem(
    "models/setfit", "all-MiniLM-L6-v2-default",
).with_embedder(multi_embedder);
```

The head's `embedding_dim` (384) must match the new encoder's output
dim. `paraphrase-multilingual-MiniLM-L12-v2` outputs 384 dim so the
default head is reusable. For other dimensions, re-train against
`queries.jsonl` (or a translated variant) using the new encoder as
`BASE_MODEL` in `scripts/train_setfit.py`.
