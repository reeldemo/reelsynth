# Grading feedback triage — Handoff Audit "mainv4"

**Date:** 19 July 2026  
**Artifact audited by grader:** claimed as manuscript `"mainv4"`  
**Artifact we actually ship:** `paper/v5/main.tex` + `paper/v5/subsections/*.tex` → `main.pdf` (DenoiseOpt / ReelSynth residual-scored hybrid RL+GA)  
**Note on filename:** A stale local copy `paper/v5/mainv5.pdf` exists and is also a DenoiseOpt build (same title/abstract). The audit text still does not describe that PDF body.  
**Verdict:** **WRONG DOCUMENT / WRONG CONTEXT.** Grade 72 / UNFIT FOR AGENT HANDOFF does not apply to our manuscript.

---

## Blunt finding

The audit describes a **RAG concatenation of OA literature PDF excerpts** (or a citation-graph expansion treated as if it were the manuscript):

- MnasNet (Tan et al.) — MobileNetV2, AmoebaNet-A 74.5%, PNASNet 74.2%, MnasNet-A1 75.2%, factorized hierarchical search, Eq.~2 latency weights
- WaveGrad (Chen et al.) — Stein score, Langevin, continuous noise-level conditioning, $y_0$ clean waveform
- Noise2Noise / Kashyap speech N2N — Theorem 2.3, $L_1$/$L_2$, losswSDR, DCUnet-20
- Elsken NAS survey bibliography internals (Zoph 2018, Chen 2018 as survey cites)

Our paper is **none of those**. It is a DenoiseOpt protocol paper: wavetable wrap / cycle-local seam restoration, prolonged residual $R$, hybrid GA+PPO+PBT outer loop, used/screened OA lit map. It briefly *cites* MnasNet, Elsken, Noise2Noise (and screens DiffWave, not WaveGrad) as priors. It does **not** synthesize MnasNet+WaveGrad+DCUnet.

Evidence from our tex (not from the graded excerpts):

| Claim in audit | In our `paper/v5`? |
|----------------|--------------------|
| MnasNet Eq.~2 / Table~1 accuracy numbers | **No** (only `\cite{tan2019mnasnet}` as platform-aware NAS prior) |
| WaveGrad / Stein / Langevin / $y_0$ | **No** (DiffWave screened as `kong2021`; no WaveGrad section) |
| DCUnet-20 / losswSDR / Theorem 2.3 | **No** |
| Dangling `[29]` Sandler / `[36]` Zoph | **No** (closed `\thebibliography`; every `\cite` key has a `\bibitem`) |
| Mixed numeric vs author-year in one doc | **No** (uniform numeric `\cite` + `cite` package) |

Cite integrity check (19 Jul 2026): **0** cited-but-missing keys, **0** bib-but-never-cited keys among in-text cites.

---

## Finding-by-finding map

| # | Audit finding | Status | Evidence / action |
|---|---------------|--------|-------------------|
| A1 | Tan/MnasNet `[29]` Sandler dangling | **IGNORE** | Wrong doc. Our bib has complete OA `\bibitem{tan2019mnasnet}` (arXiv:1807.11626). We do not cite Sandler. |
| A2 | Tan/MnasNet `[36]` Zoph dangling | **IGNORE** | Wrong doc. Our Zoph cite is `\bibitem{zoph2017nas}` (arXiv:1611.01578), cited in related work / intro, not as MnasNet's internal `[36]`. |
| A3 | Elsken survey: Zoph/Chen verified in *survey* bib | **IGNORE** | Grader scored Elsken PDF excerpt bibliography, not our paper. Our `\bibitem{elsken2019}` is complete (arXiv:1808.05377). |
| A4 | WaveGrad: Oord (2016), Ho (2020) dangling | **IGNORE** | Wrong doc. We do not cite WaveGrad, Oord, or Ho. Screened generative stack is DiffWave `\cite{kong2021}`. |
| A5 | Kashyap/N2N: `[2]` Lehtinen, `[12]` Choi/DCUnet | **IGNORE** | Wrong doc. We cite `\cite{lehtinen2018}` and `\cite{kashyap2021n2n}` as *philosophy* for unsupervised ranking, with full OA bibitems. No DCUnet. |
| A6 | Numeric vs author-year oscillation / Entity Resolution | **IGNORE** (style) | Wrong cause. Excerpts from different papers use different house styles. **Our** manuscript is consistent numeric `\cite` + `\thebibliography`. Keep numeric. Do not migrate to author-year to "fix" a concat artifact. |
| A7 | No bridge MnasNet multi-obj reward → WaveGrad diffusion | **IGNORE** | Wrong doc / invented synthesis. Not our contribution. |
| A8 | Propose MnasNet factorized search for DCUnet-20 | **IGNORE** | Do not invent. Violates "no warped MnasNet+WaveGrad synthesis" constraint. |
| A9 | Undersells MnasNet-A1 75.2% vs AmoebaNet/PNAS | **IGNORE** | ImageNet mobile NAS table is irrelevant to wavetable residual $R$. |
| A10 | Notation collision: WaveGrad $y_0$ vs N2N $y$ clean | **PARTIAL** → fixed | Collision exists only across those *other* papers. In *our* Methods we used $y$ for the engine-baked cycle and $x$ for the stored table in the intro without an explicit glossary. **Fix applied:** Methods "Notation" paragraph defining $x$, $y=\Theta(x;\theta)$, $r^{\star}$, $R$. |
| A11 | WaveGrad continuous noise-level hyperparameters | **IGNORE** | Wrong doc. |
| A12 | Extract N2N Theorem 2.3 / $L_1$/$L_2$ / losswSDR | **IGNORE** | Wrong doc. Our objective is prolonged residual $R$, not those losses. |
| A13 | Stein score / Langevin as actionable generative logic | **IGNORE** | Wrong doc. |
| A14 | Mandatory author-year BibTeX migration | **IGNORE** | Our numeric style is already coherent. Migration would be cosmetic churn, not a dangling-cite fix. |
| A15 | Re-integrate Sandler, Zoph\[36], Oord, Ho bib entries | **IGNORE** | Those keys are not missing from *our* bibliography because they are not our cites. Adding them would pad bib with unused entries and invite the false MnasNet/WaveGrad synthesis. |
| A16 | Map MnasNet $\alpha,\beta$ latency weights to WaveGrad noise conditioning | **IGNORE** | Invented bridge. Rejected. |
| A17 | Unify clean=$y$ / noisy=$x$ across WaveGrad and N2N sections | **PARTIAL** | N/A to those sections (they do not exist). **Done** via our own notation paragraph (engine symbols, not N2N symbols). |
| A18 | Factorized search vs Elsken hard-coded macro-arch / DCUnet-20 | **IGNORE** | Invented. Rejected. |
| A19 | Final grade 72 / UNFIT FOR AGENT HANDOFF | **IGNORE** | Applies to the mis-graded RAG concat, not to DenoiseOpt `paper/v5`. |

---

## Fixes applied to *our* paper (VALID/PARTIAL only)

1. **Notation paragraph** in `subsections/methods.tex`: define $x$ (stored period), $y=\Theta(x;\theta)$ (baked cycle), $r^{\star}$ (no-cliff ideal), $R$ (score). Explicitly state we do not use Noise2Noise $x$/$y$ roles.
2. **No** new bib entries for Sandler / Oord / Ho / WaveGrad.
3. **No** MnasNet–WaveGrad–DCUnet bridges.
4. **Citation style:** keep numeric `\cite` + `\thebibliography` (already consistent). Cite-key audit: closed.

---

## AUTHOR_RESPONSE (send-back paragraph)

> The Handoff Audit graded the wrong artifact. Its findings (MnasNet ImageNet tables and Eq.~2, WaveGrad Stein/Langevin and $y_0$, Noise2Noise Theorem 2.3 / losswSDR / DCUnet-20, dangling Sandler \[29] / Zoph \[36] / Oord / Ho, and mixed numeric vs author-year styles) describe a concatenation of OA literature PDF excerpts, not our DenoiseOpt manuscript (`paper/v5/main.tex`). Our paper is a residual-scored hybrid RL+GA protocol for wavetable wrap / cycle-local seam restoration. It cites MnasNet, Elsken, and Noise2Noise only as OA priors (and screens DiffWave, not WaveGrad). Every in-text `\cite` key has a matching OA `\bibitem`. Citation style is uniformly numeric. The only applicable note was clarifying our own symbols ($x$, $y$, $r^{\star}$, $R$) in Methods, which we did. We reject invented bridges (MnasNet factorized search for DCUnet-20, MnasNet latency weights to WaveGrad noise conditioning). Please re-grade `paper/v5/main.pdf` (DenoiseOpt), not the literature RAG bundle. Grade 72 / UNFIT FOR AGENT HANDOFF is vacated for this manuscript.

---

## Related: Citation Accuracy Audit

For the later score-100 **Citation Accuracy Audit** (Pixel 1 / 78 ms MnasNet; Elsken lower-resolution performance proxies), see `CITATION_ACCURACY_AUDIT_TRIAGE.md`. Those high-priority wording fixes also target lit-corpus excerpts; Pixel/78 ms claims are absent from DenoiseOpt. One optional Elsken lower-res sentence was added in Related Work.

---

## Rebuild / mirror

- Rebuild: `paper/v5/main.pdf`
- Mirror: `reelsynth/docs/papers/denoise_opt/v5/` when that tree is used (sync tex + triage + pdf)
