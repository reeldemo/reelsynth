# Public datasets for DenoiseOpt-style wrap/seam transfer

**Status:** compact companion to [`SIGNAL_HEALING_APPLICATIONS_LIT.md`](SIGNAL_HEALING_APPLICATIONS_LIT.md) §11  
**Date:** 2026-07-23  
**Scope:** Open / obtainable sci–eng datasets for cycle-local wrap experiments. Verified only — no invented collections.  
**Mirror:** [reeldemo/denoise-opt-meta](https://github.com/reeldemo/denoise-opt-meta) `docs/`

---

## Download tomorrow (ranked)

| # | Dataset | Link | License | Fit |
|---|---------|------|---------|-----|
| 1 | **CWRU bearings** | https://engineering.case.edu/bearingdatacenter | Academic open (cite CWRU) | Best default: RPM in files, 12/48 kHz, easy `.mat` |
| 2 | **MFPT bearings** | https://www.mfpt.org/fault-data-sets/ | Academic CBM (check mirrors) | Fixed **25 Hz** shaft → exact samples/rev |
| 3 | **Paderborn KAt** | https://mb.uni-paderborn.de/en/kat/research/bearing-datacenter | **CC BY-NC 4.0** | 64 kHz vib+current; Lessmeier PHME 2016 |
| 4 | **MIT-BIH ECG** | https://physionet.org/content/mitdb/1.0.0/ | **ODC-By 1.0** | Beat annotations @ 360 Hz; stitch seams |
| 5 | **PTB-XL ECG** | https://physionet.org/content/ptb-xl/1.0.3/ | **CC BY 4.0** | 12-lead scale; 500 Hz |
| 6 | **UORED-VAFCLS** | https://data.mendeley.com/datasets/y2px5tg92h | **CC BY 4.0** | 42 kHz + **Hall speed** for true COT |
| 7 | **NASA IMS bearings** | https://phm-datasets.s3.amazonaws.com/NASA/4.+Bearings.zip | U.S. gov public | Run-to-fail @ 20 kHz; healthy→fault sibling |
| 8 | **PPG-DaLiA** | https://archive.ics.uci.edu/dataset/495/ppg-dalia | **CC BY 4.0** | Wrist PPG 64 Hz + chest ECG 700 Hz |
| 9 | **KIT CNC multimodal** | https://doi.org/10.35097/hvvwn1kfwf7qt48z | **CC BY 4.0** | NC + CAD + 10 kHz sensors (best CNC proxy) |
| 10 | **IEEE 39-bus PMU OA** | https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model | DataPort OA (free account) | Streaming phasor windows / TVE |
| 11 | **PES ISS PQ capacitors** | https://site.ieee.org/pes-iss/data-sets/ | Academic non-commercial | Lab V/I under harmonics |
| 12 | **BMRB / MW FIDs** | https://bmrb.io/ · Metabolomics Workbench | Open academic | Truncate FID → apodization Θ |
| 13 | **NOAA Daily Normals** | https://www.ncei.noaa.gov/products/land-based-station/us-climate-normals | Public | Year-wrap Dec↔Jan control |

**Skip for “tomorrow”:** MIMIC-Ext-PPG (PhysioNet credentialed DUA); many IEEE DataPort PQ sets that require institutional request; CV Zenodo dumps (negative control only — iR-drop ≠ wrap).

---

## Wrap construction (one paragraph each)

**Bearings.** Use RPM (or Hall) → samples/rev \(L\). Resample to angle with a **bad** COT (1 pulse/rev, linear interp). Seam at \(0\!\leftrightarrow\!2\pi\). Ideal sibling: cubic / many-ppr resample of the **same** recording. Tile \(N\) revs; score order-spectrum residual; keep BPFO/BPFI.

**ECG/PPG.** Detect R (or systolic) peaks → extract beats → length-normalize → concatenate. Cliff = join. Sibling: SBMM / clean template; hold out arrhythmia. Score join RMSE + multi-beat tiled residual (not musical \(R\)).

**CNC.** From KIT (or synthetic CAD): export dense G01 closed contour; leave corners. Sibling: G2/G3 / NURBS blend. Prolonged = repeated toolpath; score path error + accel residual.

**PQ/PMU.** Prefer **synthetic** IEEE 1159 / C37.118.1 waveforms for raw-cycle cliffs. Use OA PMU sets for estimator-window Θ scored by TVE. Do not “pretty” the voltage.

**NMR.** Truncate a long FID; search apodization graphs vs full-length sibling spectrum (peak list + baseline wiggle).

**Climate.** Compare naive spline annual cycle vs NOAA constrained harmonics at Dec↔Jan.

---

## Top 3 starter packs

1. **Machinery:** CWRU + MFPT (+ Paderborn if NC OK).  
2. **Beats:** MIT-BIH + PTB-XL (+ PPG-DaLiA).  
3. **Probe:** KIT CNC *or* synthetic G01 + one MW/BMRB FID + NOAA normals.

Full ranked table, honesty notes, and citations: lit survey **§11**.
