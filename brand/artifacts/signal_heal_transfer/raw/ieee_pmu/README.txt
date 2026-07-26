IEEE 39-bus PMU (IEEE DataPort OA) — drop folder
================================================

S3 URI: s3://ieee-dataport/open/11968/IEEE-39-bus_10_generator_PMU.mat
Page: https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model
DOI: 10.21227/vkz3-2e96 (Naglic)

Drop the file here as:
  IEEE-39-bus_10_generator_PMU.mat

Fetch attempts
--------------
`python scripts/fetch_and_probe_ieee_pmu.py` tries:
  - boto3 / awscli S3 GET with --no-sign-request
  - public HTTPS S3 URLs
Anonymous GET currently returns **403** (OA still needs free IEEE DataPort login).
After browser download, place the .mat in this folder and re-run the probe script.

Content (important)
-------------------
This corpus is **synchrophasors** (V/I magnitude & angle, f, ROCOF, timestamps, quality)
for 10 generators × ~5197 frames over ~86.6 s — **not** raw AC oscillography.

Scoring honesty
---------------
- Preferred: TVE / window-leakage probe → cache/ieee_pmu_tve_probe.json
- Optional: `build_ieee_pmu_real()` synthesizes L=256 AC cycles from phasors + cliff
  (labeled phasor_synthesized — not native waveform prolonged-R)

Status: cache/ieee_pmu_status.json
