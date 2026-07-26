#!/usr/bin/env python3
"""Sync PhysioNet PTB-XL records500 from the AWS Open Data bucket (no sign-request).

Writes under brand/artifacts/signal_heal_transfer/raw/ptbxl_aws/records500/
and optionally flattens *.dat/*.hea into raw/ptbxl/ for build_ptbxl.
"""
from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DEST = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "raw" / "ptbxl_aws" / "records500"
FLAT = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "raw" / "ptbxl"
BUCKET = "physionet-open"
PREFIX = "ptb-xl/1.0.3/records500/"


def sync(dest: Path, max_keys: int | None = None) -> int:
    import boto3
    from botocore import UNSIGNED
    from botocore.config import Config

    dest.mkdir(parents=True, exist_ok=True)
    s3 = boto3.client("s3", config=Config(signature_version=UNSIGNED))
    paginator = s3.get_paginator("list_objects_v2")
    n_ok = 0
    n_skip = 0
    n_err = 0
    for page in paginator.paginate(Bucket=BUCKET, Prefix=PREFIX):
        for obj in page.get("Contents") or []:
            key = obj["Key"]
            if key.endswith("/"):
                continue
            rel = key[len(PREFIX) :]
            out = dest / rel
            out.parent.mkdir(parents=True, exist_ok=True)
            size = int(obj.get("Size") or 0)
            if out.is_file() and out.stat().st_size == size and size > 0:
                n_skip += 1
            else:
                try:
                    s3.download_file(BUCKET, key, str(out))
                    n_ok += 1
                except Exception as e:
                    n_err += 1
                    print(f"ERR {key}: {e}", flush=True)
            total = n_ok + n_skip
            if total % 200 == 0:
                print(f"progress downloaded={n_ok} cached={n_skip} err={n_err}", flush=True)
            if max_keys is not None and total >= max_keys:
                print(f"stopped at max_keys={max_keys}", flush=True)
                return 0 if n_err == 0 else 1
    print(f"DONE downloaded={n_ok} cached={n_skip} err={n_err} dest={dest}", flush=True)
    return 0 if n_err == 0 else 1


def flatten(dest: Path, flat: Path) -> int:
    """Copy *_hr.dat / *_hr.hea into flat raw/ptbxl (overwrite)."""
    flat.mkdir(parents=True, exist_ok=True)
    n = 0
    for p in dest.rglob("*_hr.dat"):
        shutil.copy2(p, flat / p.name)
        hea = p.with_suffix(".hea")
        if hea.is_file():
            shutil.copy2(hea, flat / hea.name)
        n += 1
    print(f"flattened {n} *_hr records into {flat}", flush=True)
    return n


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dest", type=Path, default=DEFAULT_DEST)
    ap.add_argument("--max-keys", type=int, default=None, help="optional cap for smoke")
    ap.add_argument("--flatten", action="store_true", help="also copy into raw/ptbxl")
    ap.add_argument("--flatten-only", action="store_true")
    args = ap.parse_args()
    if args.flatten_only:
        flatten(args.dest, FLAT)
        return 0
    rc = sync(args.dest, max_keys=args.max_keys)
    if args.flatten:
        flatten(args.dest, FLAT)
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
