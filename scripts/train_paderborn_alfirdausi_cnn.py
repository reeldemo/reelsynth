#!/usr/bin/env python3
"""Train Al Firdausi & Ahmad 2022 CNN_1D_2L on local Paderborn mats.

Two honest tracks (never conflated):

(A) authors_cnn_classifier — 4-class fault diagnosis as published
    (NORMAL / IR / OR / OR+IR per helper.py labels). Requires at least one
    K00*, KI*, KA*, KB* bearing folder under raw/paderborn/.

(B) alfirdausi_backbone_wrap_residual — same conv backbone as a residual
    wrap-repair head on the paderborn_kat L=256 board (NOT the author method;
    sits next to Ours/SeamN2N/DualCosine with an explicit footnote).

Seeds (documented):
  FILE_SPLIT_SEED = 20260726   # train/holdout split by .mat file
  TRAIN_SEED      = 42         # matches author notebook random_seed
  WRAP_HOLDOUT_SEED = 20260719 # same holdout as domain N2N transfer
  WRAP_TRAIN_SEED   = 424242

Citation:
  Al Firdausi, M. & Ahmad, S. (2022). Concise convolutional neural network
  model for fault detection. Comm. Sci. Tech. 7(1):62–72.
  https://github.com/mdzalfirdausi/CNN-for-Paderborn-Bearing-Dataset
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import DataLoader, TensorDataset

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from signal_heal.datasets import (  # noqa: E402
    _inject_cliff,
    _paderborn_channels,
    ensure_bundles,
)

ART = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
RAW = ART / "raw" / "paderborn"
OUT = ART / "deep_sota_adapters" / "alfirdausi_trained"
RESULTS = ART / "results_table.json"
STATUS = ART / "DEEP_SOTA_NOT_EXECUTED.json"
TABLE14 = ART / "TABLE14_NOTE_DRAFT.md"
TABLE14_STATUS = ART / "TABLE14_CAMPAIGN_STATUS.json"

SEG_LEN = 516  # author model.pth flatten=16000 ⇒ L=516
PERIOD_L = 256
# Author helper.py map_label (NOT the GUI N/B/IR/OR legend)
CLASS_NAMES = {0: "NORMAL", 1: "IR", 2: "OR", 3: "OR+IR"}
FILE_SPLIT_SEED = 20260726
TRAIN_SEED = 42
WRAP_HOLDOUT_SEED = 20260719
WRAP_TRAIN_SEED = 424242


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def flatten_size(seg_len: int) -> int:
    """Output width after two Conv+MaxPool blocks for given input length."""
    x = seg_len
    x = (x - 8) // 2  # Conv1d k=9, MaxPool1d(2)
    x = (x - 4) // 2  # Conv1d k=5, MaxPool1d(2)
    return 128 * x


class CNN_1D_2L(nn.Module):
    """Al Firdausi CNN_1D_2L reconstructed from author model.pth + gui_core."""

    def __init__(self, seg_len: int = SEG_LEN, n_classes: int = 4) -> None:
        super().__init__()
        self.seg_len = seg_len
        self.layer1 = nn.Sequential(
            nn.Conv1d(1, 64, kernel_size=9),
            nn.BatchNorm1d(64),
            nn.ReLU(),
            nn.MaxPool1d(2),
        )
        self.layer2 = nn.Sequential(
            nn.Conv1d(64, 128, kernel_size=5),
            nn.BatchNorm1d(128),
            nn.ReLU(),
            nn.MaxPool1d(2),
        )
        self.linear1 = nn.Linear(flatten_size(seg_len), n_classes)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = self.layer1(x)
        x = self.layer2(x)
        return self.linear1(x.flatten(1))


class AlfirdausiWrapResidual(nn.Module):
    """Same conv backbone as CNN_1D_2L, residual head → L-sample wrap repair.

    NOT the published method — architecture reuse only for wrap-R comparison.
    """

    def __init__(self, period_l: int = PERIOD_L) -> None:
        super().__init__()
        self.period_l = period_l
        self.layer1 = nn.Sequential(
            nn.Conv1d(1, 64, kernel_size=9),
            nn.BatchNorm1d(64),
            nn.ReLU(),
            nn.MaxPool1d(2),
        )
        self.layer2 = nn.Sequential(
            nn.Conv1d(64, 128, kernel_size=5),
            nn.BatchNorm1d(128),
            nn.ReLU(),
            nn.MaxPool1d(2),
        )
        flat = flatten_size(period_l)
        self.head = nn.Sequential(
            nn.Linear(flat, 512),
            nn.ReLU(),
            nn.Linear(512, period_l),
        )
        self.wet = nn.Parameter(torch.tensor(0.85))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # x: [B, L]
        h = x.unsqueeze(1)
        z = self.layer1(h)
        z = self.layer2(z)
        delta = self.head(z.flatten(1))
        w = self.wet.clamp(0.0, 1.0)
        return x * (1.0 - w) + (x + delta) * w


def bearing_code_from_name(name: str) -> str | None:
    """Return K00x / KIxx / KAxx / KBxx code from filename or folder name."""
    import re

    m = re.search(r"(K(?:00\d|A\d{2}|I\d{2}|B\d{2}))", name.upper())
    return m.group(1) if m else None


def label_from_filename(name: str) -> int | None:
    """Author helper.py label(): K00→NORMAL, KI→IR, KA→OR, KB→OR+IR."""
    # Match helper.py order exactly (substring checks on filename).
    if "K00" in name:
        return 0
    if "KI" in name:
        return 1
    if "KA" in name:
        return 2
    if "KB" in name:
        return 3
    return None


def discover_mat_files() -> list[tuple[Path, int, str]]:
    """[(mat_path, label, bearing_code), ...] under raw/paderborn."""
    rows: list[tuple[Path, int, str]] = []
    if not RAW.is_dir():
        return rows
    for mat in sorted(RAW.rglob("*.mat")):
        # skip nested junk under _unrar_bin etc.
        if any(p.startswith("_") for p in mat.relative_to(RAW).parts):
            continue
        lab = label_from_filename(mat.name) or label_from_filename(mat.parent.name)
        if lab is None:
            continue
        code = bearing_code_from_name(mat.name) or bearing_code_from_name(mat.parent.name) or "?"
        rows.append((mat, lab, code))
    return rows


def extract_segments(
    mats: list[tuple[Path, int, str]],
    *,
    seg_len: int,
    max_segs_per_file: int,
    max_segs_per_class: int | None,
) -> tuple[np.ndarray, np.ndarray, list[str]]:
    xs: list[np.ndarray] = []
    ys: list[int] = []
    files: list[str] = []
    per_class = {i: 0 for i in range(4)}
    for mat_path, lab, _code in mats:
        if max_segs_per_class is not None and per_class[lab] >= max_segs_per_class:
            continue
        chans = _paderborn_channels(mat_path)
        if chans is None:
            continue
        vib, *_ = chans
        vib = np.asarray(vib, dtype=np.float64).ravel()
        if vib.size < seg_len * 2:
            continue
        n_seg = min(vib.size // seg_len, max_segs_per_file)
        if max_segs_per_class is not None:
            n_seg = min(n_seg, max_segs_per_class - per_class[lab])
        for i in range(n_seg):
            w = vib[i * seg_len : (i + 1) * seg_len]
            w = (w - w.mean()) / (w.std() + 1e-8)
            xs.append(w.astype(np.float32))
            ys.append(lab)
            files.append(mat_path.name)
            per_class[lab] += 1
        if max_segs_per_class is not None and all(
            per_class[i] >= max_segs_per_class for i in range(4)
        ):
            break
    if not xs:
        return np.zeros((0, seg_len), np.float32), np.zeros((0,), np.int64), []
    return np.stack(xs, 0), np.asarray(ys, dtype=np.int64), files


def file_level_split(
    mats: list[tuple[Path, int, str]],
    *,
    holdout_frac: float,
    seed: int,
) -> tuple[list[tuple[Path, int, str]], list[tuple[Path, int, str]]]:
    """Hold out whole .mat files per class (avoids segment leakage)."""
    rng = np.random.default_rng(seed)
    by_lab: dict[int, list[tuple[Path, int, str]]] = {i: [] for i in range(4)}
    for row in mats:
        by_lab[row[1]].append(row)
    train: list[tuple[Path, int, str]] = []
    hold: list[tuple[Path, int, str]] = []
    for lab, rows in by_lab.items():
        if not rows:
            continue
        idx = rng.permutation(len(rows))
        n_hold = max(1, int(round(len(rows) * holdout_frac))) if len(rows) > 1 else 0
        hold_set = set(int(i) for i in idx[:n_hold])
        for i, row in enumerate(rows):
            (hold if i in hold_set else train).append(row)
    return train, hold


def n_params(model: nn.Module) -> int:
    return sum(p.numel() for p in model.parameters())


@torch.no_grad()
def eval_classifier(
    model: CNN_1D_2L, loader: DataLoader, device: torch.device
) -> dict[str, Any]:
    model.eval()
    preds: list[int] = []
    trues: list[int] = []
    loss_sum = 0.0
    n = 0
    for xb, yb in loader:
        xb, yb = xb.to(device), yb.to(device)
        logits = model(xb)
        loss_sum += float(F.cross_entropy(logits, yb, reduction="sum").item())
        pred = logits.argmax(dim=1)
        preds.extend(pred.cpu().tolist())
        trues.extend(yb.cpu().tolist())
        n += int(yb.numel())
    if n == 0:
        return {"status": "empty"}
    y_t = np.asarray(trues, dtype=np.int64)
    y_p = np.asarray(preds, dtype=np.int64)
    acc = float((y_t == y_p).mean())
    per_class = {}
    for i, name in CLASS_NAMES.items():
        mask = y_t == i
        if mask.any():
            per_class[name] = {
                "n": int(mask.sum()),
                "acc": float((y_p[mask] == i).mean()),
            }
        else:
            per_class[name] = {"n": 0, "acc": None}
    cm = np.zeros((4, 4), dtype=np.int64)
    for t, p in zip(y_t, y_p):
        cm[int(t), int(p)] += 1
    return {
        "n_segments": n,
        "accuracy": acc,
        "loss": loss_sum / n,
        "per_class": per_class,
        "confusion_matrix": cm.tolist(),
        "class_order": [CLASS_NAMES[i] for i in range(4)],
    }


def train_classifier(
    *,
    device: torch.device,
    epochs: int,
    batch: int,
    lr: float,
    holdout_frac: float,
    max_segs_per_file: int,
    max_segs_per_class: int,
) -> dict[str, Any]:
    mats = discover_mat_files()
    labels_present = sorted({lab for _, lab, _ in mats})
    codes = sorted({code for _, _, code in mats})
    report: dict[str, Any] = {
        "kind": "authors_cnn_classifier_trained",
        "architecture": "CNN_1D_2L",
        "seg_len": SEG_LEN,
        "class_map": CLASS_NAMES,
        "file_split_seed": FILE_SPLIT_SEED,
        "train_seed": TRAIN_SEED,
        "bearings_found": codes,
        "n_mats": len(mats),
        "labels_present": [CLASS_NAMES[i] for i in labels_present],
        "citation": {
            "paper": "Al Firdausi & Ahmad, Comm. Sci. Tech. 7(1):62–72, 2022",
            "github": "https://github.com/mdzalfirdausi/CNN-for-Paderborn-Bearing-Dataset",
        },
        "honesty": (
            "Trained from scratch on local Paderborn mats with author CNN_1D_2L arch "
            "and author helper.py labels (NORMAL/IR/OR/OR+IR). Not author model.pth. "
            "Not a wrap denoiser."
        ),
    }
    if len(labels_present) < 2:
        report["status"] = "blocked"
        report["blocker"] = (
            f"Need ≥2 fault classes for honest classification; found {labels_present} "
            f"in {RAW}. Download KI*/KA*/KB* RARs from "
            "https://groups.uni-paderborn.de/kat/BearingDataCenter/"
        )
        return report

    train_mats, hold_mats = file_level_split(
        mats, holdout_frac=holdout_frac, seed=FILE_SPLIT_SEED
    )
    x_tr, y_tr, _ = extract_segments(
        train_mats,
        seg_len=SEG_LEN,
        max_segs_per_file=max_segs_per_file,
        max_segs_per_class=max_segs_per_class,
    )
    x_ho, y_ho, _ = extract_segments(
        hold_mats,
        seg_len=SEG_LEN,
        max_segs_per_file=max_segs_per_file,
        max_segs_per_class=max_segs_per_class,
    )
    report["n_train_mats"] = len(train_mats)
    report["n_holdout_mats"] = len(hold_mats)
    report["n_train_segments"] = int(y_tr.size)
    report["n_holdout_segments"] = int(y_ho.size)
    report["train_class_counts"] = {
        CLASS_NAMES[i]: int((y_tr == i).sum()) for i in range(4)
    }
    report["holdout_class_counts"] = {
        CLASS_NAMES[i]: int((y_ho == i).sum()) for i in range(4)
    }
    if y_tr.size < 32 or y_ho.size < 8:
        report["status"] = "blocked"
        report["blocker"] = "too few segments after split"
        return report

    torch.manual_seed(TRAIN_SEED)
    np.random.seed(TRAIN_SEED)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(TRAIN_SEED)

    model = CNN_1D_2L(SEG_LEN, 4).to(device)
    opt = torch.optim.Adam(model.parameters(), lr=lr, betas=(0.99, 0.999), weight_decay=1e-5)
    train_ds = TensorDataset(
        torch.from_numpy(x_tr).unsqueeze(1), torch.from_numpy(y_tr)
    )
    hold_ds = TensorDataset(
        torch.from_numpy(x_ho).unsqueeze(1), torch.from_numpy(y_ho)
    )
    train_dl = DataLoader(train_ds, batch_size=batch, shuffle=True)
    hold_dl = DataLoader(hold_ds, batch_size=batch * 2, shuffle=False)

    hist: list[dict[str, float]] = []
    t0 = time.perf_counter()
    for ep in range(1, epochs + 1):
        model.train()
        loss_sum = 0.0
        n = 0
        for xb, yb in train_dl:
            xb, yb = xb.to(device), yb.to(device)
            opt.zero_grad(set_to_none=True)
            loss = F.cross_entropy(model(xb), yb)
            loss.backward()
            opt.step()
            loss_sum += float(loss.item()) * int(yb.numel())
            n += int(yb.numel())
        tr_loss = loss_sum / max(n, 1)
        ho = eval_classifier(model, hold_dl, device)
        hist.append({"epoch": ep, "train_loss": tr_loss, "holdout_acc": ho["accuracy"]})
        if ep == 1 or ep % max(1, epochs // 5) == 0 or ep == epochs:
            print(
                f"  [clf] epoch {ep}/{epochs} train_loss={tr_loss:.4f} "
                f"holdout_acc={ho['accuracy']:.4f}",
                flush=True,
            )

    model.eval()
    hold_metrics = eval_classifier(model, hold_dl, device)
    train_metrics = eval_classifier(model, train_dl, device)
    elapsed = time.perf_counter() - t0
    OUT.mkdir(parents=True, exist_ok=True)
    ckpt_path = OUT / "cnn_1d_2l_classifier.pt"
    torch.save(
        {
            "state_dict": {k: v.detach().cpu() for k, v in model.state_dict().items()},
            "seg_len": SEG_LEN,
            "n_classes": 4,
            "class_map": CLASS_NAMES,
            "file_split_seed": FILE_SPLIT_SEED,
            "train_seed": TRAIN_SEED,
            "epochs": epochs,
            "holdout_metrics": hold_metrics,
        },
        ckpt_path,
    )
    report.update(
        {
            "status": "trained_and_scored",
            "epochs": epochs,
            "batch": batch,
            "lr": lr,
            "n_params": n_params(model),
            "elapsed_sec": elapsed,
            "ckpt": str(ckpt_path.resolve()),
            "train_metrics": train_metrics,
            "holdout_metrics": hold_metrics,
            "history_tail": hist[-5:],
            "wrap_R": None,
            "wrap_R_note": (
                "Classifier head — wrap-R not applicable. See "
                "alfirdausi_backbone_wrap_residual for architecture-reuse bake."
            ),
        }
    )
    return report


def split_indices(n: int, holdout_n: int, seed: int) -> tuple[torch.Tensor, torch.Tensor]:
    g = torch.Generator()
    g.manual_seed(seed)
    perm = torch.randperm(n, generator=g)
    h = min(holdout_n, max(1, n // 4))
    return perm[h:], perm[:h]


@torch.no_grad()
def eval_wrap(
    model: AlfirdausiWrapResidual,
    ideal_h: torch.Tensor,
    eng_h: torch.Tensor,
) -> dict[str, float]:
    out = model(eng_h)
    return {
        "R": float(og.residual_score(ideal_h, out).mean().item()),
        "R_blend": float(og.residual_score_blend(ideal_h, eng_h, out).mean().item()),
        "no_bake_R": float(og.residual_score(ideal_h, eng_h).mean().item()),
        "dual_cosine_R": float(
            og.residual_score(ideal_h, og.dual_cosine_blend(eng_h)).mean().item()
        ),
        "n_holdout": int(ideal_h.shape[0]),
    }


def train_wrap_residual(
    *,
    device: torch.device,
    steps: int,
    batch: int,
    lr: float,
    holdout_n: int,
) -> dict[str, Any]:
    bundles = ensure_bundles(force=False, n_periods=256)
    bundle = bundles.get("paderborn_kat")
    report: dict[str, Any] = {
        "kind": "alfirdausi_backbone_wrap_residual",
        "architecture": "CNN_1D_2L_conv_backbone + residual_head",
        "period_l": PERIOD_L,
        "holdout_seed": WRAP_HOLDOUT_SEED,
        "train_seed": WRAP_TRAIN_SEED,
        "honesty": (
            "NOT the published Al Firdausi method. Reuses author conv backbone as a "
            "trainable residual wrap-repair cell on paderborn_kat for prolonged-R "
            "comparison only. Label separately from authors_cnn_classifier."
        ),
    }
    if bundle is None:
        report["status"] = "blocked"
        report["blocker"] = "paderborn_kat bundle missing"
        return report

    ideal = bundle.ideal
    engine = bundle.engine
    n = int(ideal.shape[0])
    train_idx, hold_idx = split_indices(n, holdout_n, WRAP_HOLDOUT_SEED)
    ideal_t = ideal[train_idx].to(device)
    ideal_h = ideal[hold_idx].to(device)
    eng_h = engine[hold_idx].to(device)

    torch.manual_seed(WRAP_TRAIN_SEED)
    np.random.seed(WRAP_TRAIN_SEED)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(WRAP_TRAIN_SEED)

    model = AlfirdausiWrapResidual(PERIOD_L).to(device)
    opt = torch.optim.Adam(model.parameters(), lr=lr)
    rng = np.random.default_rng(WRAP_TRAIN_SEED)
    n_train = int(ideal_t.shape[0])
    losses: list[float] = []
    t0 = time.perf_counter()
    for step in range(1, steps + 1):
        idx = torch.randint(0, n_train, (min(batch, n_train),), device=device)
        batch_ideal = ideal_t[idx]
        # Supervised residual: cliff(engine-style) → ideal (sibling-supervised bake)
        cliffs = []
        for row in batch_ideal.detach().cpu().numpy():
            cliffs.append(_inject_cliff(row, rng))
        eng = torch.tensor(np.stack(cliffs), device=device, dtype=torch.float32)
        pred = model(eng)
        loss = F.mse_loss(pred, batch_ideal)
        opt.zero_grad(set_to_none=True)
        loss.backward()
        opt.step()
        losses.append(float(loss.item()))
        if step == 1 or step % max(1, steps // 5) == 0 or step == steps:
            print(f"  [wrap] step {step}/{steps} loss={loss.item():.5f}", flush=True)

    model.eval()
    metrics = eval_wrap(model, ideal_h, eng_h)
    elapsed = time.perf_counter() - t0
    OUT.mkdir(parents=True, exist_ok=True)
    ckpt_path = OUT / "cnn_1d_2l_wrap_residual.pt"
    torch.save(
        {
            "state_dict": {k: v.detach().cpu() for k, v in model.state_dict().items()},
            "period_l": PERIOD_L,
            "protocol": "sibling_supervised_cliff_to_ideal",
            "holdout_seed": WRAP_HOLDOUT_SEED,
            "train_seed": WRAP_TRAIN_SEED,
            "steps": steps,
            "eval": metrics,
        },
        ckpt_path,
    )
    report.update(
        {
            "status": "trained_and_scored",
            "steps": steps,
            "batch": batch,
            "lr": lr,
            "n_params": n_params(model),
            "n_train": n_train,
            "elapsed_sec": elapsed,
            "ckpt": str(ckpt_path.resolve()),
            "loss_first": losses[0] if losses else None,
            "loss_last": losses[-1] if losses else None,
            "holdout": metrics,
            "wrap_R": metrics["R"],
        }
    )
    return report


def merge_results(clf: dict[str, Any], wrap: dict[str, Any]) -> None:
    if RESULTS.is_file():
        blob = json.loads(RESULTS.read_text(encoding="utf-8"))
    else:
        blob = {}
    table = blob.setdefault("table", {})
    pad = table.setdefault("paderborn_kat", {})
    pad["paderborn_deep_alfirdausi_cnn"] = {
        "kind": "authors_cnn_classifier_trained",
        "wrap_R": None,
        "holdout_accuracy": (clf.get("holdout_metrics") or {}).get("accuracy"),
        "n_holdout_segments": (clf.get("holdout_metrics") or {}).get("n_segments"),
        "epochs": clf.get("epochs"),
        "file_split_seed": FILE_SPLIT_SEED,
        "train_seed": TRAIN_SEED,
        "bearings": clf.get("bearings_found"),
        "ckpt": clf.get("ckpt"),
        "metric": "4class_holdout_accuracy_NORMAL_IR_OR_OR+IR",
        "citation": clf.get("citation"),
        "footnote": (
            "Trained Al Firdausi CNN_1D_2L from scratch on local Paderborn "
            f"(bearings={clf.get('bearings_found')}); fault classification ≠ wrap-R."
        ),
        "status": clf.get("status"),
    }
    if wrap.get("status") == "trained_and_scored":
        pad["paderborn_alfirdausi_backbone_wrap"] = {
            "kind": "alfirdausi_backbone_wrap_residual",
            "R": wrap.get("wrap_R"),
            "R_blend": (wrap.get("holdout") or {}).get("R_blend"),
            "no_bake_R": (wrap.get("holdout") or {}).get("no_bake_R"),
            "dual_cosine_R": (wrap.get("holdout") or {}).get("dual_cosine_R"),
            "steps": wrap.get("steps"),
            "holdout_seed": WRAP_HOLDOUT_SEED,
            "train_seed": WRAP_TRAIN_SEED,
            "ckpt": wrap.get("ckpt"),
            "footnote": (
                "Architecture-reuse residual bake (author conv backbone ≠ published "
                "classifier). Comparable wrap-R to Ours/SeamN2N/DualCosine."
            ),
            "status": wrap.get("status"),
        }
    skipped = blob.setdefault("skipped", {})
    skipped["paderborn_deep"] = "executed_trained_alfirdausi_cnn_classifier_plus_wrap_backbone"
    notes = blob.setdefault("notes", {})
    notes["paderborn_kat"] = (
        "K001+fault mats; classical+Ours+SeamN2N; trained Al Firdausi CNN "
        f"holdout_acc={(clf.get('holdout_metrics') or {}).get('accuracy')}; "
        f"backbone wrap-R={wrap.get('wrap_R')}"
    )
    blob["paderborn_alfirdausi_trained"] = {
        "finished_at": utc_now(),
        "classifier": {k: v for k, v in clf.items() if k != "history_tail"},
        "wrap_residual": wrap,
    }
    RESULTS.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"merged {RESULTS}", flush=True)


def update_status_json(clf: dict[str, Any], wrap: dict[str, Any]) -> None:
    if not STATUS.is_file():
        return
    blob = json.loads(STATUS.read_text(encoding="utf-8"))
    items = blob.get("items") or blob.get("baselines") or []
    note = (
        f"Trained CNN_1D_2L from scratch (not frozen author .pth); "
        f"holdout 4-class acc={(clf.get('holdout_metrics') or {}).get('accuracy')}; "
        f"epochs={clf.get('epochs')}; bearings={clf.get('bearings_found')}. "
        f"Wrap-backbone R={wrap.get('wrap_R')} (arch reuse, not author method)."
    )
    for it in items:
        if it.get("id") == "paderborn_kat_deep":
            it["status"] = "executed_trained_classifier_and_wrap_backbone"
            it["author_deep"] = {
                "classifier": clf,
                "wrap_residual": wrap,
                "updated_at": utc_now(),
            }
            it["paper_note_draft"] = note
            break
    blob["what_did_run"] = (
        (blob.get("what_did_run") or "")
        + " | Paderborn Al Firdausi CNN trained from scratch + wrap-backbone bake."
    )
    STATUS.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"updated {STATUS}", flush=True)

    if TABLE14.is_file():
        text = TABLE14.read_text(encoding="utf-8")
        line = (
            f"| Paderborn KAt deep (Al Firdausi CNN) | Trained CNN_1D_2L from scratch "
            f"(seeds file={FILE_SPLIT_SEED}/train={TRAIN_SEED}); "
            f"holdout 4-class acc={(clf.get('holdout_metrics') or {}).get('accuracy')} "
            f"(n={(clf.get('holdout_metrics') or {}).get('n_segments')}, "
            f"bearings={clf.get('bearings_found')}); "
            f"classifier wrap-R N/A. Arch-reuse wrap residual R={wrap.get('wrap_R')} "
            f"({wrap.get('steps')} steps). Wrap board: Ours 0.9270 / SeamN2N 0.8387 / "
            f"DualCosine 0.4710 |"
        )
        import re

        new_text, n = re.subn(
            r"\| Paderborn KAt deep \(Al Firdausi CNN\).*\|",
            line,
            text,
            count=1,
        )
        if n == 0:
            new_text = text.rstrip() + "\n" + line + "\n"
        TABLE14.write_text(new_text, encoding="utf-8")
        print(f"updated {TABLE14}", flush=True)

    if TABLE14_STATUS.is_file():
        st = json.loads(TABLE14_STATUS.read_text(encoding="utf-8"))
        st["paderborn_alfirdausi"] = {
            "updated_at": utc_now(),
            "classifier_holdout_acc": (clf.get("holdout_metrics") or {}).get("accuracy"),
            "wrap_backbone_R": wrap.get("wrap_R"),
            "status": "trained",
        }
        TABLE14_STATUS.write_text(json.dumps(st, indent=2), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--epochs", type=int, default=20)
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--holdout-frac", type=float, default=0.20)
    ap.add_argument("--max-segs-per-file", type=int, default=40)
    ap.add_argument("--max-segs-per-class", type=int, default=2000)
    ap.add_argument("--wrap-steps", type=int, default=4000)
    ap.add_argument("--wrap-batch", type=int, default=48)
    ap.add_argument("--wrap-lr", type=float, default=2e-3)
    ap.add_argument("--holdout-n", type=int, default=64)
    ap.add_argument("--skip-classifier", action="store_true")
    ap.add_argument("--skip-wrap", action="store_true")
    ap.add_argument("--merge", action="store_true")
    args = ap.parse_args()

    device = torch.device(args.device)
    OUT.mkdir(parents=True, exist_ok=True)
    print(f"device={device} OUT={OUT}", flush=True)

    clf: dict[str, Any] = {"status": "skipped"}
    wrap: dict[str, Any] = {"status": "skipped"}

    if not args.skip_classifier:
        print("=== (A) authors_cnn_classifier ===", flush=True)
        clf = train_classifier(
            device=device,
            epochs=args.epochs,
            batch=args.batch,
            lr=args.lr,
            holdout_frac=args.holdout_frac,
            max_segs_per_file=args.max_segs_per_file,
            max_segs_per_class=args.max_segs_per_class,
        )
        print(
            json.dumps(
                {
                    "status": clf.get("status"),
                    "holdout_acc": (clf.get("holdout_metrics") or {}).get("accuracy"),
                    "bearings": clf.get("bearings_found"),
                },
                indent=2,
            ),
            flush=True,
        )

    if not args.skip_wrap:
        print("=== (B) alfirdausi_backbone_wrap_residual ===", flush=True)
        wrap = train_wrap_residual(
            device=device,
            steps=args.wrap_steps,
            batch=args.wrap_batch,
            lr=args.wrap_lr,
            holdout_n=args.holdout_n,
        )
        print(
            json.dumps(
                {
                    "status": wrap.get("status"),
                    "wrap_R": wrap.get("wrap_R"),
                    "steps": wrap.get("steps"),
                },
                indent=2,
            ),
            flush=True,
        )

    report = {
        "updated_at": utc_now(),
        "device": str(device),
        "classifier": clf,
        "wrap_residual": wrap,
    }
    path = OUT / "train_report.json"
    path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"wrote {path}", flush=True)

    if args.merge:
        merge_results(clf, wrap)
        update_status_json(clf, wrap)

    ok = clf.get("status") == "trained_and_scored" or wrap.get("status") == "trained_and_scored"
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
