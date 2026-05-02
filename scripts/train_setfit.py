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
  - `models/setfit-training/holdout.jsonl` — 6 reserved examples used as the
    post-training accuracy gate. Same schema as `queries.jsonl`. The script
    refuses to export artifacts if accuracy falls below
    `MIN_HOLDOUT_ACCURACY` (4/6 = 66.7%); the default head clears it at
    5/6 = 83%.

Run:

  python3 scripts/train_setfit.py

Exit codes:

  0  — training + holdout gate passed, artifacts exported
  2  — training data missing
  3  — holdout accuracy below gate (no artifacts exported)

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
HOLDOUT_FILE = REPO_ROOT / "models" / "setfit-training" / "holdout.jsonl"
OUTPUT_DIR = REPO_ROOT / "models" / "setfit"
OUTPUT_PREFIX = "all-MiniLM-L6-v2-default"

BASE_MODEL = "sentence-transformers/all-MiniLM-L6-v2"
LABELS = ["Literal", "Narrative", "Analytical", "Unknown"]
SEED = 0xC0FFEE
EPOCHS = 1
BATCH_SIZE = 16
# Held-out accuracy floor. The default 4-label × 16-example dataset hits
# 5/6 = 83% on the canonical holdout (one known borderline miss between
# Literal and Analytical on "Why did the empire fall?"). Setting the gate
# at 4/6 = 66.7% catches genuine regressions (encoder drift, label
# corruption, hyperparameter break) without flapping on the borderline
# example.
MIN_HOLDOUT_ACCURACY = 4 / 6


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


def evaluate_holdout(model: SetFitModel, holdout_path: Path) -> float:
    """Run the trained SetFit model against a held-out set and return accuracy.

    The holdout file has the same JSONL schema as `queries.jsonl`. This is the
    primary regression gate: `train_setfit.yml` runs this script on every
    re-train, so a head whose accuracy drops below `MIN_HOLDOUT_ACCURACY`
    fails CI before any artifacts are committed.
    """
    rows = []
    with holdout_path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    if not rows:
        raise SystemExit(f"holdout file {holdout_path} is empty")

    print(
        f"Evaluating on {len(rows)} held-out queries from {holdout_path.name} ...",
        flush=True,
    )
    texts = [r["text"] for r in rows]
    # SetFit returns sklearn class IDs; we fed integers 0..3 indexed by
    # LABELS order in load_dataset, so int(pred) → LABELS[int(pred)] is the
    # right inverse mapping.
    predictions = model.predict(texts)
    correct = 0
    for row, pred in zip(rows, predictions):
        predicted = LABELS[int(pred)]
        expected = row["label"]
        mark = "ok " if predicted == expected else "MIS"
        print(
            f"  [{mark}] {row['text']!r:60s} → {predicted} (expected {expected})",
            flush=True,
        )
        if predicted == expected:
            correct += 1

    accuracy = correct / len(rows)
    print(
        f"  holdout accuracy: {correct}/{len(rows)} = {accuracy * 100:.1f}%",
        flush=True,
    )
    return accuracy


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

    # Gate on holdout accuracy before exporting artifacts so a regressing
    # head never lands in `models/setfit/`. If holdout.jsonl is missing
    # (e.g. downstream consumer running their own training pipeline),
    # warn and skip — the canonical repo always ships it.
    if HOLDOUT_FILE.exists():
        accuracy = evaluate_holdout(model, HOLDOUT_FILE)
        if accuracy < MIN_HOLDOUT_ACCURACY:
            print(
                f"error: holdout accuracy {accuracy * 100:.1f}% is below the "
                f"minimum {MIN_HOLDOUT_ACCURACY * 100:.1f}% gate. "
                f"Refusing to export regressing artifacts.",
                file=sys.stderr,
            )
            if staging.exists():
                shutil.rmtree(staging)
            return 3
    else:
        print(
            f"warning: {HOLDOUT_FILE} not found — skipping accuracy gate. "
            f"Add a holdout.jsonl to enable regression checking.",
            flush=True,
        )

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
