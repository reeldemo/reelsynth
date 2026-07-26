#!/usr/bin/env python3
"""Bulk-download BeatDiff Orbax prior from Drive via curl (gdown rate-limits tiny files).

Source of truth: beatdiff_prior_files.json from gdown skip_download listing.
Verifies each shard has zstd magic and decompresses.
"""
from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

import zstandard as zstd

ROOT = Path(__file__).resolve().parents[1]
WEIGHTS = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "external" / "weights" / "beatdiff"
LISTING = WEIGHTS / "_list_model" / "beatdiff_prior_files.json"
OUT = WEIGHTS / "beatdiff_prior"  # canonical target
BASELINES_SRC = WEIGHTS / "_retry" / "folder_root" / "as_folder" / "baselines"
BASELINES_DST = WEIGHTS / "baselines"


def curl_get(fid: str, dest: Path) -> bool:
    dest.parent.mkdir(parents=True, exist_ok=True)
    url = f"https://drive.google.com/uc?export=download&id={fid}&confirm=t"
    tmp = dest.with_suffix(dest.suffix + ".partial")
    cmd = [
        "curl.exe",
        "-L",
        "--fail",
        "--retry",
        "5",
        "--retry-delay",
        "3",
        "--connect-timeout",
        "30",
        "-o",
        str(tmp),
        url,
    ]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0 or not tmp.exists() or tmp.stat().st_size < 1:
        if tmp.exists():
            tmp.unlink()
        return False
    # Reject HTML error pages
    head = tmp.read_bytes()[:200]
    if b"<html" in head.lower() or b"<!doctype" in head.lower():
        tmp.unlink()
        return False
    tmp.replace(dest)
    return True


def shard_ok(path: Path) -> bool:
    if not path.is_file() or path.stat().st_size < 4:
        return False
    data = path.read_bytes()
    name = path.name.lower()
    # JSON / msgpack metadata
    if name in {"checkpoint", "metrics"} or path.suffix in {".json", ".log"}:
        return path.stat().st_size > 10 and not data[:20].lower().startswith(b"<!doctype")
    if data[:4] != b"\x28\xb5\x2f\xfd":
        # step/0 etc may still be zstd
        try:
            zstd.ZstdDecompressor().decompress(data, max_output_size=50_000_000)
            return True
        except Exception:
            return path.stat().st_size > 10
    try:
        out = zstd.ZstdDecompressor().decompress(data, max_output_size=50_000_000)
        return len(out) > 0
    except Exception:
        return False


def main() -> int:
    items = json.loads(LISTING.read_text(encoding="utf-8"))
    print(f"listing n={len(items)} out={OUT}", flush=True)
    OUT.mkdir(parents=True, exist_ok=True)

    # Copy baselines if present
    if BASELINES_SRC.is_dir():
        import shutil

        BASELINES_DST.mkdir(parents=True, exist_ok=True)
        for p in BASELINES_SRC.rglob("*"):
            if p.is_file():
                t = BASELINES_DST / p.relative_to(BASELINES_SRC)
                t.parent.mkdir(parents=True, exist_ok=True)
                if not t.exists() or t.stat().st_size < p.stat().st_size:
                    shutil.copy2(p, t)
        print(f"baselines copied -> {BASELINES_DST}", flush=True)

    ok = skip = fail = 0
    fails: list[tuple[str, str]] = []
    for i, item in enumerate(items):
        fid = item["id"]
        rel = item["path"].replace("\\", "/")
        dest = OUT / rel
        if shard_ok(dest):
            skip += 1
            if i % 50 == 0:
                print(f"[{i+1}/{len(items)}] skip {rel}", flush=True)
            continue
        # Also check old stub location
        old = WEIGHTS / "beatdiff_prior" / rel
        if dest != old and shard_ok(old):
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(old.read_bytes())
            skip += 1
            print(f"[{i+1}/{len(items)}] reuse_old {rel} size={dest.stat().st_size}", flush=True)
            continue

        success = False
        for attempt in range(4):
            if curl_get(fid, dest) and shard_ok(dest):
                success = True
                break
            time.sleep(2 * (attempt + 1))
        if success:
            ok += 1
            print(f"[{i+1}/{len(items)}] ok {rel} size={dest.stat().st_size}", flush=True)
        else:
            fail += 1
            fails.append((fid, rel))
            print(f"[{i+1}/{len(items)}] FAIL {fid} {rel}", flush=True)
            time.sleep(5)

    # Verify decompress stats
    shards = [p for p in OUT.rglob("*") if p.is_file()]
    good_zstd = 0
    total_decomp = 0
    for p in shards:
        b = p.read_bytes()
        if b[:4] == b"\x28\xb5\x2f\xfd":
            try:
                d = zstd.ZstdDecompressor().decompress(b, max_output_size=50_000_000)
                good_zstd += 1
                total_decomp += len(d)
            except Exception:
                pass
    ckpt = OUT / "model" / "7920" / "checkpoint"
    print(
        f"DONE ok={ok} skip={skip} fail={fail} n_files={len(shards)} "
        f"good_zstd={good_zstd} decomp_MB={total_decomp/1e6:.1f} "
        f"checkpoint_exists={ckpt.is_file()} checkpoint_size={ckpt.stat().st_size if ckpt.is_file() else 0}",
        flush=True,
    )
    man = WEIGHTS / "DOWNLOAD_MANIFEST.txt"
    lines = [
        "source=https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG",
        "subfolder_beatdiff_prior=https://drive.google.com/drive/folders/1QN6mZXnBpYJFxwUNYV5PXbkhd4HYw3Xh",
        f"n_listed={len(items)} n_local={len(shards)} fail={fail} good_zstd={good_zstd}",
        "",
    ]
    for fid, rel in fails:
        lines.append(f"FAIL\t{fid}\t{rel}")
    for p in sorted(shards, key=lambda x: str(x)):
        lines.append(f"{p.stat().st_size:12d}\t{p.relative_to(OUT)}")
    man.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {man}", flush=True)
    return 0 if fail == 0 else 2


if __name__ == "__main__":
    raise SystemExit(main())
