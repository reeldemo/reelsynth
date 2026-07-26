#!/usr/bin/env python3
"""Download BeatDiff Drive weights by file-ID (skip slow nested folder listing).

Parses a prior gdown folder-listing log, reconstructs paths, downloads each
file with gdown.download(id=..., resume=True). Baselines already land as .pth;
Orbax prior shards are one Drive file per array chunk.
"""
from __future__ import annotations

import re
import sys
import time
from pathlib import Path

import gdown

ROOT = Path(__file__).resolve().parents[1]
WEIGHTS = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "external" / "weights" / "beatdiff"
LOG = Path(
    r"C:\Users\Julian\.cursor\projects\c-Users-Julian-Documents-Programming-github-reeldemo-reelsynth"
    r"\terminals\144403.txt"
)
OUT = WEIGHTS / "drive_mirror"
MIN_OK = {
    ".pth": 100_000,
    ".pt": 100_000,
    ".log": 10_000,
}


def parse_tree(log_text: str) -> list[tuple[str, str]]:
    """Return list of (relative_path, file_id) from gdown listing log."""
    # Track nested folders via stack of (depth_guess, name). gdown prints DFS.
    # We approximate depth by counting consecutive Retrieving/Processing under root.
    stack: list[str] = []
    files: list[tuple[str, str]] = []
    # When we see Retrieving folder, push; we don't know when to pop cleanly.
    # Better: rebuild from path of last folders by matching known root names.
    # Use a depth heuristic: folders that look like Orbax param dirs are leaves.
    folder_stack: list[tuple[int, str]] = []  # (indent_level, name) — indent unused

    # Simpler approach: maintain stack; on Retrieving folder X:
    #   if X is known roots or baselines children, reset appropriately
    # Actually gdown prints absolute nesting — each Retrieving is enter, and
    # Processing is a file in current folder. Pop happens implicitly when a
    # sibling folder at same level appears — but log has no level markers.
    #
    # Reconstruct using the pattern from gdown source: it recurses, so after
    # Processing files in a folder it returns to parent. We only see Retrieving
    # when entering. So stack: push on Retrieving, and after Processing we stay.
    # Pop: when? We need to know folder ends. Without that, use path reconstruction
    # from file names alone for Orbax (unique leaf names) and known baseline paths.

    # Known structure from README / listing:
    #   baselines/DeScoD/channels_{1,9}/model.pth
    #   baselines/Ekgan_global_norm_{0,0_1_2}/{best_*.pth}
    #   baselines/wgan/{discriminator,generator}_trained_cl.pt
    #   beatdiff_prior/train.log
    #   beatdiff_prior/model/7920/default/<param_dir>/<chunk>

    current_folders: list[str] = []
    # Track last Retrieving chain by resetting when we hit top-level names
    TOP = {"baselines", "beatdiff_prior"}
    BASE_SUB = {"DeScoD", "Ekgan_global_norm_0", "Ekgan_global_norm_0_1_2", "wgan", "AAE"}
    DESCOD_SUB = {"channels_1", "channels_9"}

    for line in log_text.splitlines():
        m_fold = re.match(r"Retrieving folder ([A-Za-z0-9_-]+) (.+)$", line.strip())
        m_file = re.match(r"Processing file ([A-Za-z0-9_-]+) (.+)$", line.strip())
        if m_fold:
            fid, name = m_fold.group(1), m_fold.group(2).strip()
            if name in TOP:
                current_folders = [name]
            elif name == "model" and current_folders == ["beatdiff_prior"]:
                current_folders = ["beatdiff_prior", "model"]
            elif name == "7920" and current_folders[-2:] == ["beatdiff_prior", "model"]:
                current_folders = ["beatdiff_prior", "model", "7920"]
            elif name == "default" and "7920" in current_folders:
                current_folders = ["beatdiff_prior", "model", "7920", "default"]
            elif name in BASE_SUB and current_folders[:1] == ["baselines"]:
                current_folders = ["baselines", name]
            elif name in DESCOD_SUB and current_folders == ["baselines", "DeScoD"]:
                current_folders = ["baselines", "DeScoD", name]
            elif current_folders[:4] == ["beatdiff_prior", "model", "7920", "default"]:
                # Orbax param directory (one folder per array)
                current_folders = ["beatdiff_prior", "model", "7920", "default", name]
            elif name == "baselines":
                current_folders = ["baselines"]
            elif name == "beatdiff_prior":
                current_folders = ["beatdiff_prior"]
            else:
                # Unknown: append
                current_folders = current_folders + [name]
            continue
        if m_file:
            fid, name = m_file.group(1), m_file.group(2).strip()
            rel = "/".join(current_folders + [name]) if current_folders else name
            files.append((rel, fid))
    # Deduplicate keeping last
    dedup: dict[str, str] = {}
    for rel, fid in files:
        dedup[rel] = fid
    return sorted((rel, fid) for rel, fid in dedup.items())


def ok_size(path: Path) -> bool:
    if not path.is_file():
        return False
    sz = path.stat().st_size
    suf = path.suffix.lower()
    if suf in MIN_OK:
        return sz >= MIN_OK[suf]
    # Orbax chunks: accept >= 8 bytes with zstd magic; prefer > 16
    if sz < 8:
        return False
    magic = path.read_bytes()[:4]
    return magic == b"\x28\xb5\x2f\xfd" and sz >= 13


def download_one(rel: str, fid: str, dest_root: Path, retries: int = 4) -> tuple[str, str]:
    dest = dest_root / Path(rel)
    dest.parent.mkdir(parents=True, exist_ok=True)
    if ok_size(dest):
        return rel, f"skip_ok size={dest.stat().st_size}"
    # Remove tiny corrupt stubs
    if dest.exists() and dest.stat().st_size < 100_000 and dest.suffix.lower() in {".pth", ".pt"}:
        dest.unlink()
    elif dest.exists() and dest.stat().st_size < 8:
        dest.unlink()

    last_err = ""
    for attempt in range(retries):
        try:
            path = gdown.download(id=fid, output=str(dest), quiet=True, use_cookies=True, resume=True)
            if path and Path(path).is_file() and Path(path).stat().st_size >= 8:
                return rel, f"ok size={Path(path).stat().st_size} attempt={attempt}"
            last_err = f"empty_or_missing after download path={path}"
        except Exception as e:
            last_err = f"{type(e).__name__}: {e}"
        time.sleep(1.5 * (attempt + 1))
    return rel, f"FAIL {last_err}"


def main() -> int:
    log_text = LOG.read_text(encoding="utf-8", errors="ignore")
    files = parse_tree(log_text)
    print(f"parsed {len(files)} files from log", flush=True)
    OUT.mkdir(parents=True, exist_ok=True)
    # Prefer already-downloaded baselines under _retry
    retry = WEIGHTS / "_retry" / "folder_root" / "as_folder"
    if retry.is_dir():
        import shutil

        for p in retry.rglob("*"):
            if p.is_file():
                rel = p.relative_to(retry)
                target = OUT / rel
                target.parent.mkdir(parents=True, exist_ok=True)
                if not ok_size(target) or p.stat().st_size > target.stat().st_size:
                    shutil.copy2(p, target)
                    print(f"copied {rel} ({p.stat().st_size})", flush=True)

    # Also copy old train.log if present
    old_log = WEIGHTS / "beatdiff_prior" / "train.log"
    if old_log.is_file():
        t = OUT / "beatdiff_prior" / "train.log"
        t.parent.mkdir(parents=True, exist_ok=True)
        if not t.exists() or t.stat().st_size < old_log.stat().st_size:
            import shutil

            shutil.copy2(old_log, t)

    fails = []
    oks = 0
    for i, (rel, fid) in enumerate(files):
        rel2, status = download_one(rel, fid, OUT)
        print(f"[{i+1}/{len(files)}] {rel2}: {status}", flush=True)
        if status.startswith("FAIL"):
            fails.append((rel2, fid, status))
        else:
            oks += 1

    # Summary
    all_files = [p for p in OUT.rglob("*") if p.is_file()]
    big = [p for p in all_files if p.stat().st_size > 100_000]
    tiny = [p for p in all_files if p.stat().st_size <= 100]
    total = sum(p.stat().st_size for p in all_files)
    print(
        f"DONE oks={oks} fails={len(fails)} n_files={len(all_files)} "
        f"big={len(big)} tiny={len(tiny)} totalMB={total/1e6:.1f}",
        flush=True,
    )
    for rel, fid, st in fails[:40]:
        print(f"  FAIL {fid} {rel} :: {st}", flush=True)

    # Write manifest
    man = WEIGHTS / "DOWNLOAD_MANIFEST.txt"
    lines = [
        f"source=https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG",
        f"n_files={len(all_files)} total_bytes={total} fails={len(fails)}",
        "",
    ]
    for p in sorted(all_files, key=lambda x: str(x)):
        lines.append(f"{p.stat().st_size:12d}  {p.relative_to(OUT)}")
    man.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {man}", flush=True)
    return 0 if not fails else 2


if __name__ == "__main__":
    raise SystemExit(main())
