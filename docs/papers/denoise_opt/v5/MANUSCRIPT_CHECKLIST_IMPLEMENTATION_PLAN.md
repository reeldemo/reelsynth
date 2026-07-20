# Manuscript Checklist Implementation Plan — DenoiseOpt paper v5

**Date:** 19 July 2026  
**Source review:** Manuscript Checklist Review Report (22% overall / 21 FAIL per overview; 23 body `❌ FAIL` marks)  
**Artifact:** `paper/v5/main.tex` + `subsections/*.tex` → `main.pdf`  
**Companion:** `reelsynth` engine + overnight CUDA search; `docs/PSEUDOCODE.md`  
**Policy constraints (user):** OA-only cites; no resurrected long-horizon “open tables” narrative; honest numbers only; **plan only** (this document).

---

## Executive summary

The checklist grades a **stale/OCR’d PDF view** of DenoiseOpt against a full “general audio denoising” bar. Against **current v5 tex**, many structure fails are **FALSE** (Related Work, Methods, Experiments, Results, Discussion, Conclusion, numbered tables/figures, OA-badge scrub, long-horizon scrub in `9724afb`, date 19 July 2026, author `Julian~M.~Kleber`). Real gaps are **claim breadth vs evidence**, **formal Methods depth**, **evaluation matrices** (multi-waveform + SNR/SDR + stats + compute), **ablations**, and **ethics polish**.

**Recommended default (ambitious, honest):** **Narrow claims** to wavetable / cycle-local seam artifact repair **while extending scientific depth inside that domain** — SOTA methods×metrics×waveform matrices, formal R + ideal-tile defs, full algorithm environments from `PSEUDOCODE.md`, minimal-but-runnable GA/PPO/PBT/MoE ablations, multi-family holdouts (≥20 waveforms), SNR/SDR + wrap-jump, multi-seed + Wilcoxon/bootstrap, compute budget table, architecture diagram, colorblind-safe figures. Defer PESQ/STOI/MUSHRA unless the user opts in (domain mismatch on non-speech cycles). Do **not** invent theorems; only plan lemmas that follow from the residual definition.

---

## Recommended default path (strategic fork)

### Claim scope (1.1) — **narrow claims** (default)

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| **A. Narrow title/abstract** to wavetable seam / wrap discontinuity repair | Honest; matches evidence; faster peer-review defense | Weaker SEO vs “audio denoising” | **DEFAULT** |
| **B. Expand to full speech denoise benchmarks** (PESQ/STOI on speech corpora) | Satisfies checklist literally | New dataset pipeline, domain mismatch risk, weeks of work, OA speech-data licensing | Opt-in only |
| **C. Hybrid** | Narrow primary claims + cheap secondary speech-*proxy* metrics on tiled cycles | Easy to overclaim | Only if metrics stay clearly labeled as proxies |

**Default title direction** (update `main.tex` + `TITLES.md`):

> *Unsupervised Wavetable Seam Artifact Repair via Hybrid GA–PPO Meta-Search*  
> (or: *Cycle-Local Seam Restoration with Residual-Scored Hybrid RL+GA*)

Keep DenoiseOpt as method name; demote “general audio denoising” to “periodic seam artifact class.”

### Scientific extension (ambitious, inside the narrow domain)

Narrow claims ≠ thin paper. Default path **extends**:

1. **SOTA comparison matrices** (methods × metrics × waveform families).  
2. **Methods rewrite** with formal defs + search space + Algorithms.  
3. **Proofs/propositions** only where mathematically legitimate.  
4. **Ablations + multi-waveform + stats + compute**.  
5. **Results artifacts** (main matrix, ablation table, convergence, arch diagram).

**Phase order under default:** Phase 0 → **1 (claims)** → **2 (Methods/Algorithms/proofs outline)** → **4 (results artifacts from existing freezes)** in parallel with **3 (eval expansion)** → **5 (ethics)** → **6 (release gate)**. Do **not** wait for full overnight budget completion to ship v5; report frozen 5k-gate + holdout honestly.

---

## Triage summary (ground truth vs checklist)

| Bucket | Count (of 23 body FAIL marks) | Meaning |
|--------|-------------------------------|---------|
| **FALSE FAIL** | **7** | Present in current tex / already scrubbed; OCR or stale PDF |
| **PARTIAL** | **10** | Section/asset exists but depth/completeness insufficient |
| **REAL** | **6** | Genuine missing work for an ambitious revision |

Reviewer overview claimed **21 FAIL / 6 PASS**; body marks **23 FAIL**. We triage **all body FAIL marks**.

### FALSE FAIL (no substantive rewrite; verify + close)

| ID | Why false |
|----|-----------|
| **1.2** | Long-horizon “open tables” removed in `9724afb`; abstract no longer promises open mean-$R$ tables |
| **2.3** | `\input{subsections/related_work}` — dedicated Related Work with used/screened themes |
| **2.7** | `\input{subsections/discussion}` exists |
| **2.8** | `\input{subsections/conclusion}` exists |
| **2.9** | Tex uses clean arXiv `\href`s; “aXiv/[OCR” and “Access [OA]” are PDF/OCR artifacts (OA badges scrubbed). Verify cite order once |
| **3.2** | `\date{19 July 2026}` is correct in this environment (not future) |
| **4.3** | Multiple `\label{fig:*}` / `\label{tab:*}` already numbered and cross-referenced |

### PARTIAL (extend, don’t invent from zero)

| ID | What exists | What’s missing for ambitious bar |
|----|-------------|----------------------------------|
| **1.4** | Classical bake set + DualCosine + neural favorite (`tab:canonical-methods`) | Fixed MLP/CNN-on-$R$; justify DSP baseline; optional DDSP-style / wavelet as MAJOR timebox |
| **1.5** | 5-seed mean±std on canonical holdout | ≥20 diverse waveforms; Wilcoxon/bootstrap vs DualCosine |
| **2.4** | Methods + $R$ equation + notation | Ideal-tile construction; Algorithms; hyperparameter table; search-space grammar |
| **2.5** | Experiments + frozen seed protocol | Explicit protocol freeze language; multi-family dataset; eval repetitions |
| **2.6** | Results + top-5 + classical table + overnight figs | Unified SOTA matrix; ablation table; multi-family blocks |
| **2.10** | `article` + `twocolumn` + `arxiv-twocolumn.sty` | Lock venue note (arXiv preprint default); page budget target |
| **3.1** | Author `Julian~M.~Kleber` + ORCID (colon is OCR) | Add “Independent Researcher” (or institution) |
| **4.1** | Intro sine strip + many overnight/classical figures | Self-contained captions; architecture diagram; multi-waveform figure |
| **4.2** | `tab:top5`, `tab:canonical-methods` | SOTA matrix + ablation + compute tables |
| **4.4** | Plots exist | Audit axis labels/units/legends at print resolution |

### REAL (must do under default path)

| ID | Work |
|----|------|
| **1.1** | Narrow title/abstract/keywords to seam domain |
| **1.3** | Add SNR/SDR (+ wrap-jump); treat PESQ/STOI/MUSHRA as opt-in Phase 3b |
| **1.6** | Compute budget table from overnight logs |
| **2.1** | Rewrite abstract (define $R$, DualCosine, bake cell; separate holdout vs campaign) |
| **3.4** | Broader impact paragraph |
| **4.5** | Colorblind-safe palette + shape/pattern cues |

---

## Master triage table

| Checklist ID | Reviewer status | Our triage | Evidence | Action | Priority | Effort | Dependencies | Done-when |
|--------------|-----------------|------------|----------|--------|----------|--------|--------------|-----------|
| 1.1 | FAIL | **REAL** | Title/abstract say “audio denoising”; eval is sine+cliff + family probes | Narrow title/abstract/keywords; keep DenoiseOpt name | CRITICAL | S | Phase 0 claim freeze | Title+abstract match seam domain; no general denoise claim |
| 1.2 | FAIL | **FALSE FAIL** | `9724afb` scrubbed open long-horizon tables; current abstract reports 5k gate | Verify no residual “open/pending” language; do not resurrect | — | S | — | Grep clean; no open-table narrative |
| 1.3 | FAIL | **REAL** (scoped) | Only $R$ as primary; wrap-jump in dataset metrics only | Add SNR/SDR + wrap-jump to matrices; PESQ/STOI/MUSHRA → Phase 3b opt-in | CRITICAL | M–L | Waveform set (3a) | Tables report $R$+SNR/SDR(+jump); honesty note on PESQ |
| 1.4 | FAIL | **PARTIAL** | Classical set already in Results | Add MLP/CNN-on-$R$; justify DualCosine/FIR; DDSP/wavelet MAJOR optional | CRITICAL | M–L | Holdout protocol | ≥1 learned fixed baseline in SOTA matrix |
| 1.5 | FAIL | **PARTIAL** | 5-seed ±std exists | ≥20 waveforms; multi-seed; Wilcoxon/bootstrap vs DualCosine | CRITICAL | M | Diverse set | Stats table + significance vs DualCosine |
| 1.6 | FAIL | **REAL** | RTX 3090 mentioned; no hours/mem/evals table | Mine logs → compute table | MAJOR | S–M | Overnight artifacts | Table: GPU, hours, arch evals, peak mem |
| 1.7 | PASS | PASS | Intro defines wrap crackle | Keep | — | — | — | — |
| 2.1 | FAIL | **REAL** | Dense abstract; terms lightly defined | Rewrite ≤250 words; define $R$/DualCosine/bake; separate configs | CRITICAL | S | 1.1 | Abstract passes readability + term defs |
| 2.2 | PASS | PASS | Strong intro | Keep; sync claim language after 1.1 | — | S | 1.1 | Intro matches narrow claims |
| 2.3 | FAIL | **FALSE FAIL** | `related_work.tex` present | Optionally thicken hybrid-NAS contrast; no “add section from scratch” | MINOR | S | — | Section still ≥1 column; used/screened clear |
| 2.4 | FAIL | **PARTIAL** | Methods + $R$ eq | Full Methods rewrite + Algorithms + hyperparams + search space | CRITICAL | L | PSEUDOCODE | Reimplementable Methods; Algorithm 1–5 |
| 2.5 | FAIL | **PARTIAL** | Experiments section exists | Expand dataset diversity + protocol freeze language | CRITICAL | M | 3a scripts | Experiments lists waveforms, seeds, hardware |
| 2.6 | FAIL | **PARTIAL** | Results + tables/figs | SOTA matrix + ablations + multi-family blocks | CRITICAL | L | 3+4 | Main matrix + ablation published |
| 2.7 | FAIL | **FALSE FAIL** | `discussion.tex` | Extend interpretation after new matrices | MINOR | S | Phase 3–4 | Discussion cites new SOTA/ablation |
| 2.8 | FAIL | **FALSE FAIL** | `conclusion.tex` | Refresh numbers after matrices | MINOR | S | Phase 4 | Conclusion matches frozen claims |
| 2.9 | FAIL | **FALSE FAIL** | Clean `\href` arXiv IDs; `(Screened.)` labels | Rebuild PDF; cite-order audit; keep OA-only | MINOR | S | — | No OCR junk; sequential cites |
| 2.10 | FAIL | **PARTIAL** | arXiv twocolumn already | Document venue = arXiv preprint; optional later ICASSP/DAFx port | MINOR | S | User venue choice | README + plan note venue |
| 3.1 | FAIL | **PARTIAL** | Period + ORCID correct | Add Independent Researcher; ignore colon OCR | MAJOR | S | — | Affiliation line present |
| 3.2 | FAIL | **FALSE FAIL** | Today is 19 July 2026 | No change unless user wants “Submitted …” | — | — | — | Date unchanged |
| 3.3 | PASS | PASS | GitHub links | Formal reproducibility statement (seeds, deps) | MINOR | S | — | Short Reproducibility para |
| 3.4 | FAIL | **REAL** | No broader impact | Add 3–5 sentences | MINOR | S | 1.6 optional | Section present |
| 3.5 | PASS | PASS | Solo author | Optional CoI “none” | MINOR | S | — | Optional `\paragraph{COI}` |
| 4.1 | FAIL | **PARTIAL** | Many figures | Arch diagram; multi-waveform; self-contained captions | CRITICAL | M | Phase 2–3 | Captions stand alone; arch fig in |
| 4.2 | FAIL | **PARTIAL** | 2 result tables | SOTA + ablation + compute tables | CRITICAL | M | Phase 3–4 | Three new tables (+ expand main) |
| 4.3 | FAIL | **FALSE FAIL** | Numbered figs/tabs | Verify all new assets numbered | — | S | Phase 4 | Cross-refs compile |
| 4.4 | FAIL | **PARTIAL** | Plots exist | Axis/legend audit pass | MAJOR | S | Regen scripts | Print-readable labels |
| 4.5 | FAIL | **REAL** | Red-star AI cues | ColorBrewer/IBM palette + markers | MAJOR | S–M | Regen | Colorblind-safe + patterns |
| 4.6 | PASS | PASS | GitHub | Optional WAVs of DualCosine vs favorite | MINOR | S | Holdout tiles | WAV folder + README link |

---

## Mapping: reviewer CRITICAL list → our phase order

| Reviewer CRITICAL | Our phase | Notes |
|-------------------|-----------|-------|
| Complete experiments / remove open tables | **0** (verify FALSE) + honesty grep | Already scrubbed |
| ≥20 diverse waveforms | **3a** | Rust families + multi-seed `make_batch` |
| PESQ/STOI/SNR/SDR | **3a** SNR/SDR/jump; **3b** PESQ/STOI opt-in | No fake PESQ on non-speech |
| Formal algorithm + pseudocode | **2** | Integrate `docs/PSEUDOCODE.md` |
| Formal math for $R$ | **2** | Expand + ideal tile + propositions |
| Add missing IMRaD sections | **0** FALSE for presence; **2/4** deepen | Don’t rebuild empty sections |
| Ablations GA/PPO/PBT/MoE | **3c / 4** | Minimal runnable ablations |
| Main results + ablation tables | **4** (+ SOTA matrix scripts) | Ambitious matrices |

---

# Phases

## Phase 0 — Triage & ground truth (1 session)

**Goal:** Lock FALSE vs REAL; freeze evaluation protocol; confirm venue template.

### Tasks

0.1 Rebuild `paper/v5/main.pdf` from current tex; spot-check TOC/sections against checklist 2.3–2.8.  
0.2 Grep for forbidden residual language: `open until`, `remain open`, `pending`, `long horizon mean`, `Access [OA]`.  
0.3 Confirm author line uses period (`Julian~M.~Kleber`) + ORCID.  
0.4 Freeze **evaluation protocol v1** (write into Experiments later):

- Primary metric: prolonged residual $R$ (1 = best).  
- Secondary: SNR, SDR, $|wrap\ jump|$ on tiled audio.  
- Holdout seed `20260719`; search seed `1902771841`.  
- Report 5k-gate overnight freeze + frozen holdout; do not claim unfinished larger budgets.  
- Waveform diversity target: ≥20 cycles spanning Rust `sound_bench` families and/or multi-seed `make_batch` variants.

0.5 Venue: keep **arXiv twocolumn** (2.10 = PARTIAL PASS). User may later port to DAFx / ICASSP / TASLP.

0.6 Claim freeze: adopt **narrow claims + deep extension** (see Strategic fork).

**Acceptance:** Written triage counts match this plan; protocol freeze checked into `paper/v5/` (this file + optional `EVAL_PROTOCOL.md`).

**Effort:** S  

---

## Phase 1 — Honesty & claim hygiene (CRITICAL, fast)

**Goal:** Make title/abstract/intro match evidence; clear residual incomplete-result language.

### Files

- `paper/v5/main.tex` (title, abstract, keywords, `\papershorttitle`, author affiliation)  
- `paper/v5/TITLES.md`  
- `paper/v5/subsections/introduction.tex` (contribution bullets + claim language)  
- `paper/v5/subsections/conclusion.tex` (sync)

### Actions

1.1 Narrow title (default options in Strategic fork).  
1.2 Rewrite abstract (~180–220 words):

1. Problem (wrap discontinuity / seam crackle).  
2. Method (DenoiseOpt hybrid GA+PPO(+PBT)+MoE; define **bake cell**, **DualCosine**, **$R$** on first use).  
3. Frozen holdout headline ($R$ vs DualCosine).  
4. Live 5k-gate campaign headline (separate sentence).  
5. Scope sentence: cycle-local seam repair, not general speech enhancement.

1.3 Keywords: add `wavetable`, `wrap discontinuity`, `seam restoration`; demote generic “audio denoising” or qualify it.  
1.4 Author: keep period; add `\textit{Independent Researcher}` (or real affiliation).  
1.5 Remove any leftover incomplete-result phrasing (verify after 0.2).

**Acceptance:** Abstract defines $R$/DualCosine/bake; two configs separated; no general denoise overclaim.  
**Effort:** S  
**Risks:** Over-narrowing SEO — mitigate by keeping “unsupervised” + DenoiseOpt + residual-scored meta-search in abstract body.

---

## Phase 2 — Formal Methods completeness + Algorithms + proofs plan (CRITICAL, ambitious)

**Goal:** Methods become reimplementable; Algorithms in LaTeX; honest propositions only.

### 2.A Methods section rewrite outline (subsections)

Target file: `paper/v5/subsections/methods.tex` (may split into `methods_*.tex` if length warrants).

| Subsection | Content |
|------------|---------|
| **2.1 Notation** | Keep/expand $x$, $y=\Theta(x;\theta)$, $r^{\star}$, $R$; tiling length $N$; seam width |
| **2.2 Ideal tile construction** | Formal generator: same seed/family as engine input; **withhold** open-wrap cliff; optional mid-cycle noise policy; prove sibling relationship in text |
| **2.3 Residual score $R$** | Displayed equation (exists); RMS definitions; clamp; soft shape gates (if used) as optional factors |
| **2.4 Bake operator / cell** | Classical ops (DualCosine, polish, FIR, fades) + residual cells (MLP/FIR/U-Net-lite/attn-lite) + MoE gates |
| **2.5 Search space** | Discrete genome grammar: op list, depth, cell type, MoE experts; continuous $\theta$ box bounds |
| **2.6 Hybrid outer loop** | Branch rotation PPO/GA/PBT/NAS/combo; selection by $R$; plateau adapt |
| **2.7 Training / fit inner loop** | Adam on $1-R$; early stop; batch from `make_batch` |
| **2.8 Hyperparameters** | Table: pop size, tournament $k$, crossover/mutation rates, PPO $\epsilon$, lr, entropy, PBT threshold, MoE experts, plateau boredom, $N$, $L$, SEAM_W |
| **2.9 Complexity / budget argument** | Big-O per trial (fit steps × forward); why bake-width caps Demucs/DiffWave |

### 2.B Algorithm environments (LaTeX)

**Packages** (add to `main.tex`): prefer `algorithm` + `algpseudocode` (or `algorithm2e` — pick one; default **`algorithm`/`algpseudocode`** for arXiv friendliness).

| Algorithm | Source | Paper placement |
|-----------|--------|-----------------|
| Alg. 1 ResidualScore | `docs/PSEUDOCODE.md` | After §2.3 |
| Alg. 2 DualCosine baseline | PSEUDOCODE | Baselines |
| Alg. 3 FitCell (inner loop) | PSEUDOCODE | §2.7 |
| Alg. 4 GA tournament step | **expand** from PSEUDOCODE / overnight code | §2.6 |
| Alg. 5 PPO architecture update | **expand** (actions, reward $R-$DualCosine, clip) | §2.6 |
| Alg. 6 PBT exploit–mutate | **expand** | §2.6 |
| Alg. 7 MoE soft gating | **expand** (gate logits → expert mix) | §2.4 |
| Alg. 8 HybridMetaSearch | PSEUDOCODE full loop | §2.6 |
| Alg. 9 PlateauAdapt / PickFavorite | PSEUDOCODE | Optional short |

**Files:** `docs/PSEUDOCODE.md` (expand GA/PPO/PBT/MoE detail) → mirror into `methods.tex` Algorithms; keep markdown as canonical for code agents.

**Scripts:** none required beyond PDF build; optionally `scripts/export_pseudocode_check.py` later (out of scope).

### 2.C Proofs / formal arguments (honest plan — no fake theorems)

Use `\newtheorem{proposition}{Proposition}`, `\newtheorem{lemma}{Lemma}`, `\newtheorem{remark}{Remark}` (amsmath already loaded).

| ID | Statement (sketch) | Status | Plan |
|----|-------------------|--------|------|
| **Prop. R-range** | $R\in[0,1]$ by clamp construction | **Provable** | 2-line proof from definition |
| **Prop. Perfect match** | If $y_{\mathrm{tiled}}=r^{\star}_{\mathrm{tiled}}$ then $R=1$ (pre-clamp identity) | **Provable** | Direct |
| **Prop. Monotone RMS** | Strictly smaller tiled RMS error ⇒ strictly larger pre-clamp score | **Provable** | Algebra on $1-\mathrm{rms}/\mathrm{denom}$ with fixed ideal |
| **Lem. Wrap closure (sufficient)** | If cliff is the only difference between engine and ideal and a bake zeros endpoint jump *without* mid-cycle damage beyond $\varepsilon$, then $R$ nondecreases | **Conditional / sketch** | State assumptions; empirical support elsewhere |
| **Rem. N2N motivation** | Ranking without studio-clean pairs when ideal is procedural sibling | **Not a theorem** | Cite OA Noise2Noise / speech N2N as philosophy only |
| **Prop. Trial complexity** | Per-iteration cost $O(T_{\mathrm{fit}}\cdot C_{\mathrm{fwd}})$ | **Accounting argument** | Not a deep theorem — “Complexity remark” |
| ~~Thm. SOTA~~ | — | **Forbidden** | Do not invent |

**Acceptance:** Methods has Algorithms 1–8 (or clearly labeled subset), hyperparameter table, search-space definition, ≥2 short propositions with proofs; zero invented “theorems.”  
**Effort:** L  
**Risks:** Overclaiming wrap-closure lemma — keep assumptions explicit.

---

## Phase 3 — Evaluation expansion (CRITICAL, scoped; ambitious inside domain)

### Phase 3a — Waveform diversity + SNR/SDR + wrap-jump (DEFAULT)

**Goal:** ≥20 diverse waveforms; mean±std; secondary signal metrics.

**Datasets / families**

- Rust `sound_bench` families (10): e.g. harmonic_fft, am_fm, nonlinear, combo, triple_mix, extreme_overlay, open_wrap_bias, … (`src/sound_bench.rs`).  
- Multi-seed `make_batch` sine+cliff variants (seed grid).  
- Target: **≥20** scored items (e.g. 2 seeds × 10 families, or 20 `make_batch` draws + family stress block).

**Metrics (columns)**

| Metric | Domain fit | Required? |
|--------|------------|-----------|
| $R$ | Native | Yes |
| SNR / SDR (tiled vs ideal) | Honest signal quality | Yes |
| $\|x_0-x_{L-1}\|$ / wrap-jump | Seam-specific | Yes |
| Latency ms/batch, params | Already partially present | Yes in SOTA matrix |
| PESQ / STOI | Speech-oriented | **No** in 3a |
| MOS / MUSHRA | Perceptual | **No** in 3a |

**Scripts to add/extend** (paths under `reelsynth/` unless noted)

- `scripts/bench_canonical_eval_dataset.py` — extend for multi-family export.  
- New: `scripts/bench_sota_matrix.py` — methods × metrics × waveform blocks → JSON.  
- New: `scripts/metrics_snr_sdr.py` — tiled SNR/SDR helpers.  
- Optional Rust: `cargo test` / `bench_denoise_opt` family aggregation → JSON for paper.  
- `paper/v5/regen_overnight_figures.ps1` — keep for convergence figs.

**Acceptance:** JSON artifact + paper table blocks with mean±std over ≥20 waveforms; DualCosine and favorite on same tensors.  
**Effort:** M–L  

### Phase 3b — PESQ/STOI / listening (OPTIONAL, user opt-in)

- Only if waveforms are speech-like **or** user insists.  
- Must label **domain mismatch** if applied to synthetic wavetable cycles.  
- MUSHRA: MAJOR cost; not on default path.  
**Effort:** L+  

### Phase 3c — Baselines & ablations

**SOTA method rows (default matrix)**

| Row | Notes |
|-----|-------|
| identity | Ceiling / no-op |
| DualCosine | Primary classical baseline |
| seam_fir3 | Best active classical (existing) |
| classic quadratic / crossfade / hann / … | Compact classical subset or appendix |
| classical ensemble (detrend+DC+FIR) | Already scored |
| MLP-on-$R$ (fixed arch, trained on same objective) | **New** |
| CNN/U-Net-lite-on-$R$ (fixed, no outer NAS) | **New** |
| DenoiseOpt champion + top-5 tags | Existing fitted cells |

**Out of default / MAJOR timebox:** full DiffWave/Demucs (screened), learnable wavelet, heavy DDSP stacks — only if OA-citable and runnable in ≤1–2 days.

**Minimal ablations (runnable)**

| Config | Purpose |
|--------|---------|
| GA-only | Population without PPO |
| PPO-only | Policy without GA |
| GA+PPO | No PBT |
| GA+PPO+PBT | No MoE bias |
| Full (PPO+GA+PBT+NAS+depth+MoE) | Reported campaign |

Prefer **re-score / branch-best freeze** from existing `history.jsonl` where fair; run short controlled ablations if branch tags insufficient.

**Stats:** multi-seed; Wilcoxon signed-rank or bootstrap CI of $\Delta R$ vs DualCosine (paired on waveforms).

**Compute budget table:** GPU = RTX 3090; hours from overnight logs; architecture evaluations ≈ clean iterations × pop proposals; peak mem from `nvidia-smi` / PyTorch logs.

**Acceptance:** Ablation table + significance vs DualCosine + compute table drafted.  
**Effort:** M–L  
**Honesty:** Do not claim unfinished overnight as final; freeze at documented gate.

---

## Phase 4 — Results artifacts & SOTA matrices (CRITICAL, ambitious)

### 4.A SOTA comparison matrix design

**Layout options**

1. **Block tables:** one `tabular` per waveform family (or family group) with shared method rows.  
2. **Wide `table*`:** methods × metrics; families as column groups.  
3. **Heatmap figure:** methods × families for $R$ (colorblind-safe).

**Default columns:** Method | $R$ | SNR | SDR | wrap-jump | ms/batch | params | $\Delta R$ vs DualCosine  

**Generation scripts**

- `reelsynth/scripts/bench_sota_matrix.py` → `brand/artifacts/sota_matrix.json`  
- `reelsynth/scripts/plot_sota_matrix.py` → `paper/v5/figures/fig_sota_heatmap.png` (+ colorblind palette)  
- Sync JSON/PNG into `denoise-opt-meta/paper/v5/figures/`

### 4.B Paper tables/figures checklist

| Asset | Label | Source |
|-------|-------|--------|
| Main SOTA matrix | `tab:sota-main` | 3a/3c JSON |
| Ablation | `tab:ablation` | 3c |
| Compute | `tab:compute` | overnight logs |
| Hyperparameters | `tab:hyperparams` | Phase 2 |
| Top-5 (keep) | `tab:top5` | existing |
| Classical (keep or merge) | `tab:canonical-methods` | existing |
| Convergence | `fig:champ-residual` etc. | regen script |
| Architecture diagram | `fig:denoiseopt-arch` | **new** (TikZ or SVG→PDF) |
| Multi-waveform strip | `fig:multi-family` | new |
| Intro sine (keep) | `fig:intro-sine` | expand caption |

### 4.C Captions & accessibility

- Self-contained captions (seed, $N$, batch, what $R$ means).  
- ColorBrewer / IBM colorblind-safe; markers/linestyles not color-only.  
- Axis labels with units.

**Files:** `subsections/results.tex`, `discussion.tex`, `figures/*`, regen scripts.  
**Acceptance:** PDF shows SOTA + ablation + compute; arch diagram referenced in Methods/Results; captions readable alone.  
**Effort:** L  

---

## Phase 5 — Ethics polish (MAJOR/MINOR)

| Item | Action | Effort |
|------|--------|--------|
| Affiliation | Independent Researcher | S |
| Broader impact | VA/instruments; low misuse; synthetic data; GPU-hours | S |
| Reproducibility | Seeds, deps (`torch`, CUDA, Rust), script entrypoints | S |
| Optional CoI | “None to declare” | S |
| WAV examples | DualCosine vs favorite on holdout tiles → GitHub `artifacts/audio_examples/` | S |

**Files:** `main.tex`, new short `subsections/ethics.tex` or paragraphs in tooling/limitations; audio under `reelsynth/brand/artifacts/` or meta `artifacts/`.  

---

## Phase 6 — Integration & release gate

6.1 `pdflatex` ×2; fix overfull boxes from Algorithms/tables.  
6.2 Update **v5 only** — do not resurrect long-horizon open tables.  
6.3 Sync mirror: `reelsynth/docs/papers/denoise_opt/v5/` (tex + this plan + key figures).  
6.4 Commit + push `denoise-opt-meta` and mirror.  
6.5 Re-score checklist on **real** items only — target **≥80%** of REAL+PARTIAL items addressed (FALSE FAILs counted as already closed).  
6.6 Optional: author response paragraph citing OCR/stale-PDF false fails (like `GRADING_FEEDBACK_TRIAGE.md`).

**Acceptance:** Plan executed checklist; PDF builds; repos pushed.  

---

## Per-REAL/PARTIAL fix cards (concrete)

### Card A — Narrow claims + abstract (1.1, 2.1)

- **Edit:** `main.tex`, `TITLES.md`, `introduction.tex`  
- **Run:** `pdflatex`  
- **Effort:** S  
- **Done-when:** Title/abstract seam-scoped; terms defined  
- **Risk:** SEO — keep unsupervised + DenoiseOpt in abstract  

### Card B — Methods + Algorithms + propositions (2.4)

- **Edit:** `methods.tex`, `main.tex` (packages/theorems), `docs/PSEUDOCODE.md`  
- **Run:** `pdflatex`  
- **Effort:** L  
- **Done-when:** Alg. ResidualScore + Hybrid + GA/PPO/PBT/MoE present; hyperparam table; Prop. R-range proved  
- **Risk:** Pseudocode drift from code — cite overnight script path  

### Card C — SOTA matrix + SNR/SDR + ≥20 waveforms (1.3, 1.5, 4.2)

- **Edit/add:** `bench_sota_matrix.py`, `metrics_snr_sdr.py`, `results.tex`  
- **Run:** matrix bench; plot heatmap; ingest JSON  
- **Effort:** L  
- **Done-when:** ≥20 waveforms; $R$/SNR/SDR/jump; mean±std  
- **Risk:** Do not invent PESQ on non-speech  

### Card D — Learned baselines + ablations (1.4, 2.6)

- **Edit/add:** fixed MLP/CNN trainers; ablation runner or history re-agg  
- **Effort:** M–L  
- **Done-when:** Matrix rows + `tab:ablation`  
- **Risk:** Unfair training budgets — match fit-steps to FitCell  

### Card E — Stats + compute (1.5, 1.6)

- **Edit:** `experiments.tex`, `results.tex`; log miner script  
- **Effort:** S–M  
- **Done-when:** Wilcoxon/bootstrap + compute table  
- **Risk:** Underpowered $n$ — report effect size $\Delta R$ even if $p$ marginal  

### Card F — Figures accessibility + arch diagram (4.1, 4.4, 4.5)

- **Edit:** plot scripts palette; TikZ/SVG arch; captions  
- **Run:** `regen_overnight_figures.ps1`; new plot scripts  
- **Effort:** M  
- **Done-when:** Colorblind-safe; arch fig; caption audit  

### Card G — Ethics (3.1, 3.4, 3.3 note)

- **Edit:** `main.tex` / `ethics` / `tooling.tex`  
- **Effort:** S  
- **Done-when:** Affiliation + broader impact + reproducibility para  

---

## Open decisions for the user

1. **PESQ/STOI?** Default **no** (Phase 3b opt-in).  
2. **Listening test / MUSHRA?** Default **no** (MAJOR).  
3. **Venue?** Default **arXiv twocolumn**; later DAFx/ICASSP template port.  
4. **Affiliation string?** Default **Independent Researcher**.  
5. **Title final wording?** Pick among narrow options in Strategic fork / `TITLES.md`.  
6. **DDSP / wavelet baselines?** Default **defer** (MAJOR); MLP/CNN-on-$R$ in scope.  
7. **How ambitious are ablations?** Prefer short controlled runs + honest branch-best from 5k freeze vs multi-day re-search.

---

## Constraints (do not violate)

- OA-only bibliography; no paywalled-only cites.  
- No long-horizon “tables remain open” narrative.  
- No invented PESQ/STOI on non-speech without labeling.  
- No unfinished overnight claimed as complete.  
- No fake theorems; propositions only from definitions/assumptions.  
- Plan-only until user authorizes implementation phases.

---

## Effort rollup (default ambitious path)

| Phase | Effort | Criticality |
|-------|--------|-------------|
| 0 Triage | S | Required |
| 1 Claims | S | CRITICAL |
| 2 Methods/Algs/Props | L | CRITICAL |
| 3a Diversity+SNR/SDR | M–L | CRITICAL |
| 3b PESQ/MUSHRA | L+ | Optional |
| 3c Baselines/ablations/stats/compute | M–L | CRITICAL |
| 4 SOTA artifacts/figs | L | CRITICAL |
| 5 Ethics | S | MAJOR/MINOR |
| 6 Release | S | Required |

**Suggested sprint:** Phase 0–1 (½ day) → Phase 2 skeleton + Algorithms (1–2 days) → Phase 3a/3c scripts overnight → Phase 4 tables/figs (1 day) → Phase 5–6 (½ day).

---

## Related docs

- `GRADING_FEEDBACK_TRIAGE.md` — wrong-document handoff audit (ignore for this checklist).  
- `CITATION_ACCURACY_AUDIT_TRIAGE.md` — citation wording.  
- `docs/PSEUDOCODE.md` — algorithm source of truth to integrate.  
- Commit `9724afb` — OA tag + long-horizon scrub.  

---

## Appendix — FALSE FAIL one-liners for author response

> Several checklist fails describe a stale/OCR PDF: Related Work, Methods, Experiments, Results, Discussion, and Conclusion are already `\input` sections; result tables and numbered figures exist; long-horizon open tables and OA access badges were removed in `9724afb`; the author name uses a period and ORCID (colon is OCR); the date 19 July 2026 is the actual drafting date. Remaining work is claim narrowing, Methods/Algorithms depth, multi-waveform SOTA matrices with SNR/SDR, ablations, stats, compute reporting, and ethics polish—not rebuilding absent IMRaD sections.
