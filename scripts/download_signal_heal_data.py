#!/usr/bin/env python3
"""Download raw data for the signal-heal transfer pilot.

Fetches CWRU (Zenodo mirror), MIT-BIH + PTB-XL subsets (PhysioNet), and attempts
MFPT / Paderborn OA mirrors. HTML walls and login gates are documented, not faked.
"""
from __future__ import annotations

import json
import ssl
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RAW = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "raw"
CACHE = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "cache"
CTX = ssl.create_default_context()

CWRU = {
    "cwru/97.mat": ["https://zenodo.org/records/10986655/files/97.mat?download=1"],
    "cwru/98.mat": ["https://zenodo.org/records/10986655/files/98.mat?download=1"],
    "cwru/105.mat": ["https://zenodo.org/records/10986655/files/105.mat?download=1"],
    "cwru/169.mat": ["https://zenodo.org/records/10986655/files/169.mat?download=1"],
    "cwru/209.mat": ["https://zenodo.org/records/10986655/files/209.mat?download=1"],
}

# Official often returns HTML; try OA mirrors (Figshare / data-acoustics / MathWorks GH).
MFPT = {
    "mfpt/MFPT-Fault-Data-Sets.zip": [
        "https://www.mfpt.org/wp-content/uploads/2020/02/MFPT-Fault-Data-Sets.zip",
        "https://figshare.com/ndownloader/files/53049140",
        "https://ndownloader.figshare.com/files/53049140",
        "http://data-acoustics.com/wp-content/uploads/2014/10/MFPT-Fault-Data-Sets.zip",
    ],
}

# Paderborn KAt — usually registration; probe a few public hints only.
PADERBORN_PROBE = [
    "https://mb.uni-paderborn.de/fileadmin/fakultaeten/mb/nachrichten/kat/bearingdatacenter/K001.rar",
    "https://groups.uni-paderborn.de/kat/BearingDataCenter/K001.rar",
]

MITDB_RECS = ["100", "101", "103", "105", "112", "113", "115", "117", "121", "123"]

# PTB-XL 100 Hz low-res subset (legacy pilot).
PTBXL_RECS_LR = [f"{i:05d}_lr" for i in range(1, 21)]

# PTB-XL 500 Hz high-res (records500). Prefer AWS Open Data sync via
# scripts/sync_ptbxl_records500.py (full corpus ~2–3 GB). HTTP fallback below
# pulls a large contiguous OA slice when AWS is unavailable.
# Folders are record_id // 1000 → 00000, 00001, …
# Policy (see build_ptbxl / PTBXL_POLICY.md): lead I only, *_hr @ 500 Hz,
# R–R beat windows, board n=256 from the downloaded pool.
PTBXL_HR_N = 5000  # HTTP fallback: records 00001_hr … 05000_hr (~substantially larger than pilot 200)


def try_get(urls: list[str], dest: Path, min_bytes: int = 1000) -> tuple[bool, str]:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.is_file() and dest.stat().st_size >= min_bytes:
        head = dest.read_bytes()[:32]
        if dest.suffix.lower() == ".zip" and not head.startswith(b"PK"):
            dest.unlink(missing_ok=True)
        else:
            print(f"exists {dest} ({dest.stat().st_size})")
            return True, "cached"
    notes: list[str] = []
    for u in urls:
        for unverified in (False, True):
            try:
                print(f"GET {u} unverified={unverified}")
                ctx = ssl._create_unverified_context() if unverified else CTX
                req = urllib.request.Request(u, headers={"User-Agent": "reelsynth-signal-heal/0.2"})
                with urllib.request.urlopen(req, context=ctx, timeout=120) as r:
                    data = r.read()
                if len(data) < min_bytes:
                    notes.append(f"{u}: too small {len(data)}")
                    print(f"  too small {len(data)} (need {min_bytes})")
                    continue
                if dest.suffix.lower() == ".zip" and not data.startswith(b"PK"):
                    notes.append(f"{u}: HTML/login wall (not PK zip)")
                    print("  not a zip (HTML/login wall?) — skip")
                    continue
                if dest.suffix.lower() == ".mat" and data[:6].lstrip().startswith(b"<!DOC"):
                    notes.append(f"{u}: HTML masquerading as mat")
                    print("  HTML not mat — skip")
                    continue
                dest.write_bytes(data)
                print(f"OK {dest} ({len(data)})")
                return True, u
            except Exception as e:
                notes.append(f"{u}: {type(e).__name__}: {e}")
                print(f"  fail {type(e).__name__}: {e}")
    return False, "; ".join(notes[:6])


def dl_mitdb() -> bool:
    dest = RAW / "mitdb"
    dest.mkdir(parents=True, exist_ok=True)
    try:
        import wfdb
    except ImportError:
        wfdb = None
    ok = True
    for rec in MITDB_RECS:
        base = f"https://physionet.org/files/mitdb/1.0.0/{rec}"
        for ext, amin in ((".dat", 1000), (".hea", 40), (".atr", 40)):
            path = dest / f"{rec}{ext}"
            if path.is_file() and path.stat().st_size >= amin:
                continue
            got, _ = try_get([base + ext], path, min_bytes=amin)
            if not got:
                if wfdb is not None:
                    try:
                        wfdb.dl_files("mitdb", str(dest), [f"{rec}{ext}"])
                    except Exception as e:
                        print(f"wfdb fail {rec}{ext}: {e}")
                        ok = False
                else:
                    ok = False
    return ok


def dl_ptbxl() -> bool:
    """Download PTB-XL records100 (legacy) + records500 (500 Hz) slice into raw/ptbxl/."""
    dest = RAW / "ptbxl"
    dest.mkdir(parents=True, exist_ok=True)
    ok = True
    for rec in PTBXL_RECS_LR:
        base_nested = f"https://physionet.org/files/ptb-xl/1.0.3/records100/00000/{rec}"
        for ext, amin in ((".dat", 1000), (".hea", 40)):
            path = dest / f"{rec}{ext}"
            if path.is_file() and path.stat().st_size >= amin:
                continue
            got, _ = try_get([base_nested + ext], path, min_bytes=amin)
            if not got:
                ok = False

    hr_ok = 0
    for i in range(1, PTBXL_HR_N + 1):
        rec = f"{i:05d}_hr"
        folder = f"{(i // 1000):05d}"
        base_nested = f"https://physionet.org/files/ptb-xl/1.0.3/records500/{folder}/{rec}"
        rec_ok = True
        for ext, amin in ((".dat", 5000), (".hea", 40)):
            path = dest / f"{rec}{ext}"
            if path.is_file() and path.stat().st_size >= amin:
                continue
            got, _ = try_get([base_nested + ext], path, min_bytes=amin)
            if not got:
                rec_ok = False
                ok = False
        if rec_ok and (dest / f"{rec}.dat").is_file():
            hr_ok += 1
    print(f"PTB-XL records500 (_hr) downloaded/cached: {hr_ok}/{PTBXL_HR_N}")
    if hr_ok < 32:
        ok = False
    return ok


def probe_mfpt() -> dict:
    dest = RAW / "mfpt" / "MFPT-Fault-Data-Sets.zip"
    got, note = try_get(list(MFPT.values())[0], dest, min_bytes=10_000)
    return {
        "ok": got,
        "path": str(dest) if got else None,
        "note": note if not got else f"downloaded from {note}",
    }


def probe_paderborn() -> dict:
    """No login flow in this session — probe only; document skip."""
    notes: list[str] = []
    for u in PADERBORN_PROBE:
        got, note = try_get([u], RAW / "paderborn" / Path(u).name, min_bytes=1000)
        if got:
            return {"ok": True, "url": u, "note": "probe hit"}
        notes.append(note)
    return {
        "ok": False,
        "note": (
            "skipped — Paderborn KAt Bearing Data Center requires registration / "
            "no anonymous OA mirror hit in this session; "
            + "; ".join(notes[:3])
        ),
    }


def main() -> int:
    RAW.mkdir(parents=True, exist_ok=True)
    CACHE.mkdir(parents=True, exist_ok=True)
    skip_log: dict[str, str] = {}

    cwru_ok = all(try_get(urls, RAW / rel)[0] for rel, urls in CWRU.items())
    print("CWRU ok=", cwru_ok)

    mfpt = probe_mfpt()
    print("MFPT ok=", mfpt["ok"], mfpt.get("note", "")[:200])
    if not mfpt["ok"]:
        skip_log["mfpt_bearings"] = (
            "MFPT zip: official + Figshare + data-acoustics mirrors returned HTML wall / "
            f"timeout / 404 — {mfpt.get('note', '')[:300]}"
        )

    pad = probe_paderborn()
    print("Paderborn ok=", pad["ok"])
    if not pad["ok"]:
        skip_log["paderborn_kat"] = pad["note"]

    mit_ok = dl_mitdb()
    print("MITDB ok=", mit_ok)

    ptb_ok = dl_ptbxl()
    print("PTB-XL (incl. records500 500 Hz slice) ok=", ptb_ok)
    if not ptb_ok:
        skip_log["ptbxl_ecg"] = "PTB-XL PhysioNet 500 Hz / 100 Hz download incomplete"

    # Optional OA sets: KIT drop folder; IEEE PMU open S3 (often 403 anonymously)
    kit_dir = RAW / "kit_cnc"
    kit_dir.mkdir(parents=True, exist_ok=True)
    kit_has = any(kit_dir.rglob("*.mat")) or any(kit_dir.rglob("*.json")) or any(
        kit_dir.rglob("*.nc")
    )
    if kit_has:
        skip_log["kit_cnc"] = "files detected under raw/kit_cnc/ — build_kit_cnc_real available"
    else:
        skip_log.setdefault(
            "kit_cnc",
            "awaiting drop under raw/kit_cnc/ (kit_cnc_README.txt); synth_cnc_g01 proxy",
        )

    ieee_dir = RAW / "ieee_pmu"
    ieee_dir.mkdir(parents=True, exist_ok=True)
    ieee_mat = list(ieee_dir.glob("*.mat"))
    if not ieee_mat:
        try:
            probe = ROOT / "scripts" / "fetch_and_probe_ieee_pmu.py"
            if probe.is_file():
                subprocess.run([sys.executable, str(probe)], check=False)
                ieee_mat = list(ieee_dir.glob("*.mat"))
        except Exception as e:
            skip_log["ieee_pmu_fetch_error"] = str(e)
    if ieee_mat:
        skip_log["ieee_pmu"] = (
            f"mat present ({ieee_mat[0].name}); phasors — run fetch_and_probe_ieee_pmu.py"
        )
    else:
        skip_log.setdefault(
            "ieee_pmu",
            "S3 URI known but anonymous GET 403; drop .mat into raw/ieee_pmu/; "
            "synth_pmu_cycle proxy; prefer TVE probe when present",
        )
    skip_log.setdefault("bmrb_nmr", "skipped — BMRB FID deferred")

    (CACHE / "download_status.json").write_text(
        json.dumps(
            {
                "cwru": cwru_ok,
                "mitdb": mit_ok,
                "ptbxl": ptb_ok,
                "mfpt": mfpt,
                "paderborn": pad,
                "skipped": skip_log,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    (CACHE / "skipped_optional.json").write_text(json.dumps(skip_log, indent=2), encoding="utf-8")
    return 0 if (cwru_ok and mit_ok) else 1


if __name__ == "__main__":
    raise SystemExit(main())
