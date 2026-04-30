#!/usr/bin/env python3
"""Train and export the default SetFit head shipped with tsumugi.

PR-2 of the LLM-removal series. Produces three artifacts under
`models/setfit/`:

  - `all-MiniLM-L6-v2-default.onnx`       — encoder (sentence-transformers
                                            mean-pooled, L2-normalized,
                                            384-dim output)
  - `all-MiniLM-L6-v2-default.tokenizer.json`
  - `all-MiniLM-L6-v2-default.head.json`  — schema matches
                                            `tsumugi_core::classifier::LinearHeadFile`

Inputs:

  - `models/setfit-training/queries.jsonl` — 4 labels × 16 examples = 64 rows.
    Each row is `{"text": "...", "label": "Literal|Narrative|Analytical|Unknown"}`.

Run:

  python3 scripts/train_setfit.py

The script is deterministic (fixed seed) so re-running with the same
training data should produce a bit-identical head JSON. The encoder ONNX
isn't bit-identical between runs because Optimum's export is non-
deterministic in the topological order of operators, but its weights are
identical. tsumugi-core only consumes the encoder embedding, so this
non-determinism doesn't affect runtime behaviour.

Dependencies (CPU-only training is fine):

  pip install \
    setfit==1.1.0 \
    transformers==4.46.0 \
    optimum[onnxruntime]>=1.20 \
    torch==2.4.1 \
    datasets

`transformers >= 4.47` removed the `CallbackHandler.tokenizer` attribute
that setfit 1.1.0's `Trainer` initialiser still touches, so the pin is
required as of late 2026. setfit 1.2+ drops the dependency, so once
that ships you can lift the pin.

CPU training time: ~1-2 min for 16 examples × 4 labels on Apple M1 / x86_64.
"""

from __future__ import annotations

import json
import os
import random
import shutil
import sys
from pathlib import Path

import numpy as np
import torch
from datasets import Dataset
from optimum.onnxruntime import ORTModelForFeatureExtraction
from setfit import SetFitModel, Trainer, TrainingArguments
from transformers import AutoTokenizer

REPO_ROOT = Path(__file__).resolve().parent.parent
TRAINING_FILE = REPO_ROOT / "models" / "setfit-training" / "queries.jsonl"
OUTPUT_DIR = REPO_ROOT / "models" / "setfit"
OUTPUT_PREFIX = "all-MiniLM-L6-v2-default"

BASE_MODEL = "sentence-transformers/all-MiniLM-L6-v2"
LABELS = ["Literal", "Narrative", "Analytical", "Unknown"]
SEED = 0xC0FFEE
EPOCHS = 1
BATCH_SIZE = 16


def set_seed(seed: int) -> None:
    random.seed(seed)
    np.random.seed(seed)
    torch.manual_seed(seed)
    torch.cuda.manual_seed_all(seed)


def load_dataset(path: Path) -> Dataset:
    rows = []
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    label_to_id = {lbl: i for i, lbl in enumerate(LABELS)}
    texts = [r["text"] for r in rows]
    labels = [label_to_id[r["label"]] for r in rows]
    return Dataset.from_dict({"text": texts, "label": labels})


def export_encoder_onnx(model: SetFitModel, out_dir: Path, prefix: str) -> tuple[Path, Path]:
    """Export the SetFit encoder (sentence-transformers MiniLM) to ONNX.

    Optimum's ORTModelForFeatureExtraction handles the mean-pooling +
    L2 normalize when configured with the right pooling head, but tsumugi's
    OnnxEmbedding does mean-pool + L2 normalize itself. So we export the
    raw encoder (last_hidden_state) and let tsumugi do the pooling.
    """
    encoder_path = model.model_body[0].auto_model.config._name_or_path
    tmp_dir = out_dir / f"{prefix}.optimum-export"
    if tmp_dir.exists():
        shutil.rmtree(tmp_dir)
    tmp_dir.mkdir(parents=True, exist_ok=True)

    print(f"Exporting encoder ONNX from {encoder_path} ...", flush=True)
    ort_model = ORTModelForFeatureExtraction.from_pretrained(
        encoder_path, export=True
    )
    ort_model.save_pretrained(tmp_dir)
    AutoTokenizer.from_pretrained(encoder_path).save_pretrained(tmp_dir)

    onnx_target = out_dir / f"{prefix}.onnx"
    tokenizer_target = out_dir / f"{prefix}.tokenizer.json"
    shutil.copy(tmp_dir / "model.onnx", onnx_target)
    shutil.copy(tmp_dir / "tokenizer.json", tokenizer_target)
    shutil.rmtree(tmp_dir)
    print(f"  → {onnx_target}", flush=True)
    print(f"  → {tokenizer_target}", flush=True)
    return onnx_target, tokenizer_target


def export_head_json(model: SetFitModel, out_dir: Path, prefix: str) -> Path:
    """Export the linear head (sklearn LogisticRegression) as JSON.

    Schema matches `tsumugi_core::classifier::LinearHeadFile`:

      {
        "labels": [...],
        "embedding_dim": 384,
        "weights": [[...384 floats...], ...],   # row-major [num_labels][emb_dim]
        "bias":    [...num_labels floats...]
      }
    """
    head = model.model_head
    classes = head.classes_.tolist()
    # SetFit's default head uses sklearn LogisticRegression. Some versions
    # wrap it in OneVsRest, some don't. Handle both cleanly.
    if hasattr(head, "estimators_"):
        # OneVsRestClassifier of binary LogisticRegression — one estimator
        # per class. coef_ is (1, emb_dim) per estimator.
        weights = np.vstack([est.coef_.flatten() for est in head.estimators_])
        bias = np.array([est.intercept_[0] for est in head.estimators_])
    else:
        # Plain (multi-class) LogisticRegression: coef_ is (num_classes, emb_dim).
        weights = np.asarray(head.coef_)
        bias = np.asarray(head.intercept_)
    if weights.shape[0] != len(LABELS):
        raise SystemExit(
            f"head produced {weights.shape[0]} class rows, expected {len(LABELS)}"
        )
    embedding_dim = int(weights.shape[1])

    # Reorder rows so that row i corresponds to LABELS[i] regardless of
    # what order sklearn happened to assign internal class indices.
    label_idx_remap = [int(c) for c in classes]
    reordered = np.empty_like(weights)
    reordered_bias = np.empty_like(bias)
    for sklearn_row, label_id in enumerate(label_idx_remap):
        reordered[label_id] = weights[sklearn_row]
        reordered_bias[label_id] = bias[sklearn_row]

    payload = {
        "labels": LABELS,
        "embedding_dim": embedding_dim,
        "weights": reordered.astype(np.float32).tolist(),
        "bias": reordered_bias.astype(np.float32).tolist(),
    }
    head_target = out_dir / f"{prefix}.head.json"
    with head_target.open("w") as f:
        json.dump(payload, f, indent=2)
    print(f"  → {head_target}", flush=True)
    return head_target


def main() -> int:
    if not TRAINING_FILE.exists():
        print(f"error: {TRAINING_FILE} not found", file=sys.stderr)
        return 2
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    set_seed(SEED)
    print(f"Loading training set from {TRAINING_FILE} ...", flush=True)
    train_ds = load_dataset(TRAINING_FILE)
    print(f"  {len(train_ds)} rows, {len(set(train_ds['label']))} labels", flush=True)

    print(f"Loading base model {BASE_MODEL} ...", flush=True)
    model = SetFitModel.from_pretrained(BASE_MODEL)
    args = TrainingArguments(
        batch_size=BATCH_SIZE,
        num_epochs=EPOCHS,
        seed=SEED,
        output_dir=str(OUTPUT_DIR / "setfit-trainer-staging"),
    )
    trainer = Trainer(model=model, args=args, train_dataset=train_ds)
    print("Fine-tuning encoder + fitting linear head (CPU 1-2 min) ...", flush=True)
    trainer.train()
    # SetFit Trainer leaves a `setfit-trainer-staging/` artefact behind. We
    # only want the head + encoder ONNX, so prune the dir at the end.
    staging = OUTPUT_DIR / "setfit-trainer-staging"

    export_encoder_onnx(model, OUTPUT_DIR, OUTPUT_PREFIX)
    export_head_json(model, OUTPUT_DIR, OUTPUT_PREFIX)

    if staging.exists():
        shutil.rmtree(staging)

    print("Done. Artifacts ready under models/setfit/ — git add + commit.", flush=True)
    print(
        "Note: `*.onnx` files are LFS-tracked via `.gitattributes`; "
        "`git lfs install` must have been run in this clone for the commit "
        "to upload pointers correctly.",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
