#!/usr/bin/env python3
"""Download raw data for the signal-heal transfer pilot.

Fetches CWRU (Zenodo mirror), MIT-BIH + PTB-XL subsets (PhysioNet), and attempts
MFPT / Paderborn OA mirrors. HTML walls and login gates are documented, not faked.
"""
from __future__ import annotations

import json
import ssl
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

# PTB-XL 100 Hz low-res subset — enough beats for L=256 wrap pilot without full dump.
PTBXL_RECS = [f"{i:05d}_lr" for i in range(1, 21)]


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
    """Download a small PTB-XL records100 subset into flat raw/ptbxl/."""
    dest = RAW / "ptbxl"
    dest.mkdir(parents=True, exist_ok=True)
    ok = True
    for rec in PTBXL_RECS:
        # PhysioNet layout: records100/00000/NNNNN_lr.{dat,hea}
        # Also accept already-flat sibling downloads.
        base_nested = f"https://physionet.org/files/ptb-xl/1.0.3/records100/00000/{rec}"
        for ext, amin in ((".dat", 1000), (".hea", 40)):
            path = dest / f"{rec}{ext}"
            if path.is_file() and path.stat().st_size >= amin:
                continue
            got, _ = try_get([base_nested + ext], path, min_bytes=amin)
            if not got:
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
    print("PTB-XL subset ok=", ptb_ok)
    if not ptb_ok:
        skip_log["ptbxl_ecg"] = "PTB-XL PhysioNet subset incomplete after download"

    # Always note login-walled optional sets for paper honesty
    skip_log.setdefault(
        "kit_cnc",
        "skipped — KIT CNC DOI needs browser/login flow; synthetic_cnc_wrap used as proxy",
    )
    skip_log.setdefault(
        "ieee_pmu",
        "skipped — IEEE DataPort free-account wall; synthetic_power_wrap used as proxy",
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
