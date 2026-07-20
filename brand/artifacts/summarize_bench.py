import json
from pathlib import Path

fit = json.loads(Path("brand/artifacts/denoise_opt_bench_100k_fit.json").read_text())
print("fitted overall", fit["overall"])
print("fitted theta", [round(x, 3) for x in fit["fitted_theta"]])
print("prev frozen", [round(x, 3) for x in fit["previous_frozen"]])
fams = sorted(fit["per_family"], key=lambda r: -r["quality"])
print("best:")
for r in fams[:3]:
    print(f"  {r['family']}: q={r['quality']:.3f} d={r['denoise']:.3f} s={r['shape']:.3f}")
print("worst:")
for r in fams[-3:]:
    print(f"  {r['family']}: q={r['quality']:.3f} d={r['denoise']:.3f} s={r['shape']:.3f}")
