# Citation Accuracy Audit triage — mainv4.pdf

**Date:** 19 July 2026  
**Audit label:** Citation Accuracy Audit (score-100 style OA lit check)  
**Artifact audited by grader:** claimed as `"mainv4.pdf"`  
**Artifact we ship:** `paper/v4/main.tex` → DenoiseOpt (wavetable seam residual / hybrid RL+GA)  
**Verdict:** High-priority Pixel-1 / MnasNet-78ms and ImageNet-table corrections **do not apply** — those claims are absent from our manuscript. One optional Elsken sentence added where Related Work already discusses NAS taxonomy.

---

## Blunt finding

Same failure mode as `GRADING_FEEDBACK_TRIAGE.md`: the auditor grades **OA literature RAG excerpts** (MnasNet arXiv:1807.11626, Elsken survey arXiv:1808.05377, WaveGrad, Kashyap/N2N), not DenoiseOpt.

### High-priority corrections from the audit

| # | Audit correction | In our `paper/v4` body? | Action |
|---|------------------|-------------------------|--------|
| H1 | MnasNet **78 ms** latency must say **Pixel 1** (not generic “Pixel phone”) | **No** — zero hits for `78ms`, `78 ms`, `Pixel`, `75.2`, MobileNetV2 ImageNet tables | **Nothing to patch.** Do not invent an MnasNet ImageNet/Pixel paragraph to satisfy the auditor. |
| H2 | NAS performance estimation must note **lower-resolution images** as a fidelity proxy (Elsken) | **Partial** — we already cite Elsken for search space / strategy / evaluation cost; we did **not** previously name lower-res proxies | **Optional natural fix applied:** one OA-cited sentence in Related Work §NAS (see below). |

Unverifiable claims beyond Pixel 1 / Nvidia K80: **out of audit scope** — ignored (we make no such claims).

### Grep evidence (19 Jul 2026)

Searched `paper/v4/**/*.tex` for: MnasNet, Pixel, 78ms, MobileNet, WaveGrad, Noise2Noise, DCUnet, Langevin, Elsken, performance estimation, lower-resolution, ImageNet, 75.2.

| Term | Hits in DenoiseOpt body |
|------|-------------------------|
| Pixel / 78ms / 75.2 / ImageNet (as MnasNet result) | **0** |
| WaveGrad / Langevin / DCUnet | **0** (DiffWave screened as `kong2021` only) |
| MnasNet | cite-only: platform-aware NAS prior `\cite{tan2019mnasnet}` |
| Elsken | taxonomy + (now) lower-fidelity / lower-resolution proxies |
| Noise2Noise | philosophy for unsupervised residual ranking only |

---

## Optional fix applied (H2 only)

In `subsections/related_work.tex`, after the existing Elsken taxonomy sentence:

> Their performance-estimation axis includes lower-fidelity proxies such as shorter training, data subsets, and lower-resolution images~\cite{elsken2019}.

Ground truth: Elsken et al.\ survey §4 / Table 1 (arXiv:1808.05377 OA PDF) lists lower-resolution images among lower-fidelity performance estimates.

**Not done (would invent wrong content):**

- MnasNet-A1 75.2% / AmoebaNet / PNAS tables  
- “78 ms on Pixel 1” latency claim  
- WaveGrad Stein/Langevin / Noise2Noise Theorem 2.3 / DCUnet-20 results sections  

---

## Relation to prior handoff triage

See `GRADING_FEEDBACK_TRIAGE.md` for the earlier “Handoff Audit” (dangling Sandler/Zoph/Oord/Ho, invented MnasNet↔WaveGrad bridges). Same wrong-document diagnosis. This file covers only the Citation Accuracy Audit’s two high-priority wording items.

---

## Rebuild / mirror

- Rebuild: `paper/v4/main.pdf`
- Mirror: `reelsynth/docs/papers/denoise_opt/v4/`
