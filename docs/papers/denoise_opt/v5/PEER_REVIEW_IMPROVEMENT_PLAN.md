# DenoiseOpt v5 Peer-Review Improvement Plan

> **Structure:** This plan follows the **writing-plans** skill layout: phased workstreams, checkbox tasks, exact files, effort, dependencies, done-when acceptance criteria, and risks.  
> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.  
> **Plan only.** Do not implement until the user explicitly starts execution.

**Goal:** Answer a peer-review-style critique of DenoiseOpt v5 by deepening evaluation honesty, baselines, wavetable-native realism, and theory clarity, while keeping claims narrowly scoped to cycle-local wavetable seam repair.

**Architecture:** Keep the existing DenoiseOpt hybrid (GA+PPO+PBT+NAS) and prolonged residual $R$ as the primary story. Extend depth inside that scope: cliff-stratified reporting, true self-supervised seam restorers (primary corrupt→corrupt N2N), ReelSynth-exported + OA instrument/WT cycles under the wrap-discontinuity protocol, and honest theory deletion (no Lemma 1 / Props theater). Do not pivot to general speech enhancement SOTA.

**Tech Stack:** Python/PyTorch benches in `reelsynth/scripts/`; Rust `sound_bench` + `export_sound_bench_tiles`; paper TeX in `denoise-opt-meta/paper/v5/` mirrored to `reelsynth/docs/papers/denoise_opt/v5/`.

**Canonical plan path:** `denoise-opt-meta/paper/v5/PEER_REVIEW_IMPROVEMENT_PLAN.md`  
**Mirror:** `reelsynth/docs/papers/denoise_opt/v5/PEER_REVIEW_IMPROVEMENT_PLAN.md`  
**Critique date:** 19 July 2026  
**Source critique:** peer-review paste (Strengths / Weaknesses / Suggestions) on DenoiseOpt v5 manuscript

## Global Constraints

- **Narrow claims only:** cycle-local wavetable / wrap-seam artifact repair. Never claim general speech or music enhancement SOTA.
- **OA-only cites:** every `\cite` / `\bibitem` must resolve to an open-access source already audited or newly OA-verified.
- **No fake PESQ on sine:** PESQ/STOI remain out of scope for non-speech tiles. Do not invent speech-metric numbers on synthetic cycles.
- **MUSHRA ignored** unless the user later explicitly requests a listening study.
- **No em-dash slop** in future prose notes, abstracts, or discussion edits (use periods or short clauses).
- **Speech/music LibriSpeech/MUSDB probes default OFF.** Wavetable-native realism first.
- **Holdout seed** `20260719`; overnight search seed `1902771841` (never thousand-separated in prose).
- **Primary metric remains** prolonged residual $R\in[0,1]$; secondary SNR/SDR/wrap-jump; optional seam-local metrics must be explicitly defined.

---

## Executive summary

The critique is partly venue mismatch (NeurIPS-style “real speech/music + modern DL SOTA”) and partly valid scientific pressure (identity $R$ looks too easy, wrap-jump barely moves, theory is thin, no true Noise2Noise baseline, synthetic-only evaluation).

**Locked stance:** stay niche and deepen. Treat DenoiseOpt as a DSP / audio-engineering contribution with a hybrid search protocol, not a general audio ML SOTA paper. Answer valid points with:

1. **Cliff-stratified reporting** so hard wrap cases carry the claim.
2. **Stronger learned baselines** (true N2N-style + lightweight seq model) on the same generator.
3. **Wavetable-native realism** (ReelSynth exports + OA instrument one-shots under wrap protocol), not a LibriSpeech bait-and-switch.
4. **Theory honesty** (delete Lemma 1 and trivial Props theater; keep formal $R$; no wrap-closure or search-convergence guarantees).
5. **Writing / venue repositioning** toward DAFx / AES / arXiv DSP rather than top ML impact theater.

**Recommended first phase:** Phase A (metric & baseline honesty). Fast, high ROI, unblocks abstract/discussion updates, and reframes the identity-$R$ critique without new training runs.

---

## Locked decisions (19 July 2026 grill)

Grill frame: fix **what is scientifically weak**, not what merely *looks* weak to reviewers. Venue stays **DAFx / AES / arXiv DSP**. Do not start Phase A–E implementation until the user explicitly starts execution.

| # | Topic | Locked choice |
|---|-------|---------------|
| 1 | **Lemma 1 / theory** | **DELETE** Lemma 1 and trivial Props theater. Keep the formal definition of residual $R$. Add an explicit sentence: **no wrap-closure or search-convergence guarantees**. Epistemic honesty over decorative theory. |
| 2 | **N2N baselines** | **FULL stress test.** **Primary:** corrupt→corrupt N2N on the same `make_batch` geometry. **Secondary:** sibling-supervised ceiling. **No holdout tile leakage into training.** |
| 3 | **Real samples** | **BOTH.** **Primary realism:** ReelSynth-exported periods (same engine). **Secondary:** license-clean OA instrument / WT one-shots under the same wrap protocol. **No LibriSpeech / MUSDB.** |
| 4 | **Venue** | Stay DAFx / AES / arXiv DSP (already locked stance; reaffirmed). |
| 5 | **Seam-local metric** | Still open: edge RMSE (recommended default) and/or click energy. Not decided in this grill. |

### Still open (not grilled)

- [ ] Optional seam-local metric: edge RMSE and/or click energy (default recommendation remains edge RMSE).
- [ ] Exact DAFx year / AES track vs arXiv-only timing (venue *class* is locked; submission target date is not).

---

## Critique triage map

Every weakness and suggestion is classified as **REAL** (must fix), **PARTIAL** (address within wavetable scope), or **REJECT** (venue mismatch / out of locked stance).

### Weaknesses

| ID | Critique item | Verdict | Action |
|----|---------------|---------|--------|
| W1 | Eval confined to synthetic single-cycle waveforms; limits top-ML impact | **PARTIAL** | Phase C: add real instrument / ReelSynth wavetable cycles under wrap protocol. Keep synthetic as primary controlled corpus. Reject “must do LibriSpeech to matter.” |
| W2 | No eval on real instrument / speech / polyphonic audio | **PARTIAL** / **REJECT split** | **PARTIAL:** real instruments + exported WT cycles (Phase C). **REJECT:** speech/polyphony as primary claim domain. OOD speech probe stays optional and OFF by default. |
| W3 | Honest “no general speech/music SOTA” limits contribution | **REJECT** (as a defect) | Keep honesty. Soften overclaim language only; reposition venue (Phase E). Narrow claims are a feature for DAFx/AES. |
| W4 | Props 1–2 trivial for ML audience | **REAL** | Phase D: **DELETE** Props theater; at most fold into $R$ definition prose (locked 19 Jul grill). |
| W5 | Prop 3 self-evident monotone map | **REAL** | Phase D: fold into $R$ definition paragraph only; no standalone “theorem theater” (locked). |
| W6 | Lemma 1 is only a sketch; not a real lemma | **REAL** | Phase D: **DELETE** Lemma 1; explicit “no wrap-closure / search-convergence guarantees” sentence (locked). |
| W7 | Identity $R$ deceptively high (~0.97 / ~0.965) | **REAL** | Phase A: explain residual-mass vs ideal RMS; stratify by cliff; make hard-cliff the operative regime in Results/Discussion. |
| W8 | Wrap-jump barely changes (0.91 → 0.90) while $R$ jumps | **REAL** | Phase A: report wrap-jump + optional edge RMSE / click energy on hard subset; clarify that favorite may repair tiled residual without fully zeroing endpoint jump; avoid claiming wrap-jump SOTA if numbers stay flat. |
| W9 | Cite Noise2Noise philosophy but no N2N-style baseline | **REAL** | Phase B: **primary** corrupt→corrupt N2N + **secondary** sibling-supervised ceiling; no holdout tile leakage (locked). |

### Suggestions

| ID | Suggestion | Verdict | Action |
|----|------------|---------|--------|
| S1 | Test on real-world WT recordings or standard music/speech sets | **PARTIAL** | Phase C: **both** ReelSynth-exported periods (primary) + OA instrument/WT one-shots (secondary). **REJECT** LibriSpeech/MUSDB (locked). |
| S2 | LibriSpeech / MUSDB for “relevance” | **REJECT** | Locked: no LibriSpeech/MUSDB. Document in Phase E “What we will NOT do.” |
| S3 | Modern DL baseline: small CNN/RNN self-supervised on seam problem | **REAL** | Phase B: full N2N stress test (primary corrupt→corrupt) + lightweight LSTM/1D-CNN seq model (locked). |
| S4 | Replace props with search convergence / error-landscape analysis | **PARTIAL** | Phase D: delete weak theory; optional honest complexity/budget note only if non-fake; **no** invented guarantees (locked). |
| S5 | Prove Lemma 1 conditions or remove | **REAL** | Phase D: **DELETE** (locked; same as W6). |
| S6 | Report top 10% high-cliff cases separately | **REAL** | Phase A: top 10% and top 25% wrap-jump / cliff-magnitude strata. |
| S7 | Add polynomial fitter / LSTM / SSM baselines | **REAL** / **PARTIAL** | Phase B: LSTM or small 1D CNN required; polynomial fitter optional small add if cheap; full SSM optional if timeboxed. |

### Strengths to preserve (do not regress)

| Strength | Keep / reinforce |
|----------|------------------|
| Clear cycle-local seam problem framing | Keep title/abstract claim freeze |
| Hybrid GA+PPO+PBT+NAS under one $R$ objective | Keep as method contribution |
| Modular Algorithms 1–8 | Keep; only clarify search vs fit vs score |
| Frozen eval protocol, seeds, compute, open source | Extend protocol with strata docs |
| Stats (Wilcoxon, multi-seed ±std, ablations) | Reuse on hard-cliff subsets |
| Classical DualCosine / FIR retained | Keep; document why Demucs/DiffWave screened |

---

## What we will NOT do

- Full **LibriSpeech / MUSDB / general speech enhancement SOTA** chase.
- Invented **PESQ/STOI** on sine or non-speech wavetable tiles.
- **MUSHRA** listening panels unless user later requests.
- Fake **convergence theorems** for hybrid GA+PPO+PBT search.
- Pivoting abstract to “general audio denoising” or NeurIPS-impact cosplay.
- Training **full Demucs / DiffWave** as primary baselines (screened as compute/domain mismatch; document why).
- Em-dash-heavy prose rewrites.

---

## Suggested venue positioning

| Venue class | Fit | Notes |
|-------------|-----|-------|
| **DAFx** | Best primary target | DSP problem, bake operators, seam metrics, hybrid search as engineering method |
| **AES** (convention / journal short) | Strong secondary | Wavetable / synthesis artifact repair audience |
| **arXiv cs.SD / eess.AS** | Always | DSP-framed preprint; honest narrow abstract |
| **Top ML (NeurIPS/ICML/ICLR)** | Poor fit unless reframed | Critique’s impact complaint stands under current narrow scope; do not chase without speech/music SOTA (which we refuse) |

Phase E updates Related Work + Discussion to state this positioning explicitly.

---

## Priority order for execution

1. **Phase A** — Metric & baseline honesty (fast, high ROI)  
2. **Phase D** — Theory cleanup (can parallelize with A; mostly TeX)  
3. **Phase B** — Stronger learned baselines (needs training compute)  
4. **Phase C** — Wavetable-native realism (sample scope locked: both)  
5. **Phase E** — Writing / positioning (after A numbers; refresh again after B/C)

Parallelization note: A and D can run in the same sprint. B and C can overlap (sample source + N2N scope + Lemma 1 fate are locked).

---

## Open decisions for user

Grill-locked items (1–3, venue class) are recorded under **Locked decisions (19 July 2026 grill)** above. Remaining:

- [x] ~~**Real sample pack source?**~~ → **BOTH** (ReelSynth-exported primary; OA instrument/WT secondary). No LibriSpeech/MUSDB.
- [x] ~~**Noise2Noise baseline scope?**~~ → **FULL stress test** (primary corrupt→corrupt; secondary sibling-supervised). No holdout tile leakage.
- [x] ~~**Lemma 1 fate?**~~ → **DELETE** + no wrap-closure / search-convergence guarantees. Delete trivial Props theater; keep formal $R$.
- [ ] **Optional seam-local metric?** Edge RMSE and/or click energy on tiled wrap region. Default recommendation: **edge RMSE** (simplest, reproducible).
- [ ] **Venue submission timing?** Venue *class* locked (DAFx / AES / arXiv DSP). Exact year / track / arXiv-only timing still open.

---

## File map (shared across phases)

| Path | Role |
|------|------|
| `reelsynth/scripts/overnight_gpu_rl_arch.py` | `make_batch`, residual $R$, overnight search |
| `reelsynth/scripts/bench_sota_matrix.py` | Multi-family SOTA matrix |
| `reelsynth/scripts/bench_canonical_eval_dataset.py` | Canonical holdout corpus stats |
| `reelsynth/scripts/metrics_snr_sdr.py` | SNR/SDR/wrap-jump helpers |
| `reelsynth/scripts/bench_rust_sound_bench_tiles.py` | Rust tile transfer bench |
| `reelsynth/src/sound_bench.rs` | Rust procedural families |
| `reelsynth/src/bin/export_sound_bench_tiles.rs` | Export 20 Rust tiles JSON |
| `denoise-opt-meta/paper/v5/subsections/methods.tex` | Props/Lemma/$R$ definition |
| `denoise-opt-meta/paper/v5/subsections/experiments.tex` | Protocol, dataset |
| `denoise-opt-meta/paper/v5/subsections/results.tex` | Tables, strata |
| `denoise-opt-meta/paper/v5/subsections/discussion.tex` | Identity-$R$ / hard-cliff narrative |
| `denoise-opt-meta/paper/v5/subsections/limitations.tex` | Scope honesty |
| `denoise-opt-meta/paper/v5/subsections/related_work.tex` | N2N contrast |
| `denoise-opt-meta/paper/v5/main.tex` | Abstract / keywords |
| `denoise-opt-meta/paper/v5/EVAL_PROTOCOL.md` | Frozen protocol extension |
| `denoise-opt-meta/paper/v5/figures/*.json` | Regenerated figure/table blobs |
| Mirror tree | `reelsynth/docs/papers/denoise_opt/v5/` (sync tex + this plan) |

---

# Phase A — Metric & baseline honesty

**Effort:** S–M  
**Dependencies:** None (uses existing tensors / benches)  
**ROI:** Highest. Directly answers W7, W8, S6.

### Intent

- Stratify by wrap-jump / cliff magnitude (top 10% and top 25% hardest tiles).
- Report $R$, SNR/SDR, wrap-jump for identity vs favorite (and DualCosine) on hard subsets.
- Clarify in text why identity $R$ can be high (small residual mass relative to ideal RMS) and that hard-cliff is the operative regime.
- Optional: secondary seam-local metric (edge RMSE).

### Tasks

#### Task A1: Cliff-stratum scoring script

**Files:**
- Create: `reelsynth/scripts/bench_cliff_strata.py`
- Modify: `reelsynth/scripts/metrics_snr_sdr.py` (export helpers if needed)
- Create: `denoise-opt-meta/paper/v5/figures/cliff_strata.json`
- Mirror copy of JSON under `reelsynth/docs/papers/denoise_opt/v5/figures/`

**Interfaces:**
- Consumes: `overnight_gpu_rl_arch.make_batch`, favorite bake cfg from overnight freeze, DualCosine, identity
- Produces: JSON with keys `all`, `top25_wrap`, `top10_wrap` each containing per-method `{R_mean, R_std, snr_mean, sdr_mean, wrap_jump_mean, n}`

- [ ] **Step 1:** Draw a large holdout batch (e.g. 4096 cycles, seed `20260719`) with engine + ideal; compute per-tile wrap-jump and cliff amplitude.
- [ ] **Step 2:** Define strata thresholds as empirical percentiles of wrap-jump (document exact cutoffs in JSON `meta`).
- [ ] **Step 3:** Score identity / DualCosine / favorite / `seam_fir3` on each stratum; write `cliff_strata.json`.
- [ ] **Step 4:** Unit sanity: assert `top10` mean wrap-jump > `top25` > `all`; assert `n` counts match.
- [ ] **Step 5:** Commit script + JSON (no TeX yet).

**Done-when:** JSON exists; hard strata show identity $R$ drop and/or favorite $\Delta R$ vs DualCosine larger than on `all`. If not, document that honestly (still publish strata).

**Risks:** Hard subset may be small; use at least 256 tiles in top 10% or widen draw. Do not cherry-pick cutoffs after seeing favorite numbers.

#### Task A2: Optional edge RMSE / click energy

**Files:**
- Modify: `reelsynth/scripts/metrics_snr_sdr.py`
- Modify: `reelsynth/scripts/bench_cliff_strata.py`
- Modify: `denoise-opt-meta/paper/v5/EVAL_PROTOCOL.md`

- [ ] **Step 1:** Define `edge_rmse` = RMS of `(y - r*)` on indices `[0:SEAM_W] U [L-SEAM_W:L]` after bake (document formula in EVAL_PROTOCOL).
- [ ] **Step 2:** Optional `click_energy` = mean square of first difference across the tiled wrap boundary (samples at $kL-1$ and $kL$).
- [ ] **Step 3:** Add both to stratum JSON; mark optional in TeX until numbers are stable.

**Done-when:** Metric definitions are unambiguous and reproducible from JSON + protocol text.

**Risks:** Click energy can correlate with intentional bright saw content; restrict narrative to wrap-local indices.

#### Task A3: Manuscript text + table for strata

**Files:**
- Modify: `denoise-opt-meta/paper/v5/subsections/results.tex`
- Modify: `denoise-opt-meta/paper/v5/subsections/discussion.tex`
- Modify: `denoise-opt-meta/paper/v5/subsections/experiments.tex`
- Modify: `denoise-opt-meta/paper/v5/main.tex` (abstract hard-cliff clause once numbers land)
- Sync mirror tex

- [ ] **Step 1:** Add Table `tab:cliff-strata` (all / top25 / top10) with identity, DualCosine, favorite columns for $R$ and wrap-jump (SNR/SDR if space).
- [ ] **Step 2:** Add one Results paragraph: identity $R$ is high because residual mass ≪ ideal RMS on mild cliffs; operative claim is hard-cliff.
- [ ] **Step 3:** Discussion: address wrap-jump flatness honestly; if favorite does not reduce wrap-jump much, claim tiled residual repair, not endpoint-zeroing.
- [ ] **Step 4:** Rebuild PDF; sync mirror.

**Acceptance criteria (Phase A):**
- [ ] Top 10% and top 25% strata reported in paper + JSON.
- [ ] Explicit prose explaining high identity $R$.
- [ ] No new PESQ/MUSHRA.
- [ ] Abstract/discussion mention hard-cliff regime once measured.

**Risks:** Abstract update too early before JSON freezes; freeze cutoffs in EVAL_PROTOCOL first.

---

# Phase B — Stronger learned baselines

**Effort:** M–L  
**Dependencies:** Phase A protocol freeze helpful but not strictly required; same generator as overnight.  
**Answers:** W9, S3, S7.

### Intent

- Add a **true Noise2Noise-style / self-supervised seam restorer** trained on the same generator, distinct from current “CNN-on-$R$” meta-fit.
- **Locked (19 Jul grill):** full stress test — **primary** `n2n_corrupt_corrupt` on same `make_batch` geometry; **secondary** sibling-supervised ceiling; **no holdout tile leakage into training**.
- Add a **lightweight LSTM or small 1D CNN seq model** predicting ideal from cracked tile.
- Keep classical DualCosine/FIR; document why full Demucs/DiffWave remain screened.

### Tasks

#### Task B1: N2N-style baseline trainer

**Files:**
- Create: `reelsynth/scripts/train_n2n_seam_baseline.py`
- Create: `reelsynth/scripts/baselines/n2n_seam.py` (model + train/eval API)
- Create: `denoise-opt-meta/paper/v5/figures/n2n_baseline.json`
- Modify: `reelsynth/scripts/bench_sota_matrix.py` (register method)

**Interfaces:**
- Consumes: `make_batch` / family batch API producing `(ideal, engine)` or two independent corruptions
- Produces: checkpoint + eval row `{method, R, snr, sdr, wrap_jump, ms, params}`

Training modes (**both required**; name clearly; primary vs secondary as locked):
1. **`n2n_corrupt_corrupt` (PRIMARY):** two independent cliff draws from same mid-cycle seed → predict one from the other (true N2N spirit), same `make_batch` geometry.
2. **`n2n_sibling_supervised` (SECONDARY ceiling):** engine → ideal sibling (oracle proxy; label as supervised sibling, not “unsupervised search”).

Constraints:
- No outer NAS / PPO / access to overnight champion search state.
- **No holdout tile leakage into training** (train seeds disjoint from eval/holdout seed `20260719`).
- Fixed architecture (small 1D CNN or U-Net-lite), fixed Adam, fixed step budget documented in JSON.
- Same $L{=}256$, seed policy documented.

- [ ] **Step 1:** Implement model + train loop with train seeds disjoint from holdout `20260719` / held-out eval seeds.
- [ ] **Step 2:** Smoke train 500 steps; assert loss decreases; assert eval $R$ > DualCosine on canonical holdout or document failure.
- [ ] **Step 3:** Full train (budget in JSON, e.g. 20k–50k steps); export metrics for **both** modes.
- [ ] **Step 4:** Add rows to SOTA matrix + cliff strata script; primary row = corrupt→corrupt; secondary = sibling-supervised.
- [ ] **Step 5:** Commit checkpoints metadata (paths) + JSON; large `.pt` via release/artifact policy already used by overnight models.

**Done-when:** Both N2N modes appear in Results tables (primary corrupt→corrupt emphasized), clearly distinguished from CNN-on-$R$ meta-fit and from DenoiseOpt favorite; training/eval tile sets disjoint.

**Risks:** Sibling-supervised may beat favorite and weaken meta-search story; report honestly. Corrupt-corrupt may be weak; still the primary baseline.

#### Task B2: Lightweight seq baseline (LSTM or 1D CNN)

**Files:**
- Create: `reelsynth/scripts/baselines/seq_seam_lstm.py` (or `seq_seam_cnn1d.py`)
- Create: `reelsynth/scripts/train_seq_seam_baseline.py`
- Modify: `bench_sota_matrix.py`, `bench_cliff_strata.py`

- [ ] **Step 1:** Choose LSTM (~1–2 layers, hidden ≤64) **or** depthwise 1D CNN; document param count target ≤100k.
- [ ] **Step 2:** Train engine→ideal on same generator; evaluate on canonical + 20-family + hard strata.
- [ ] **Step 3:** Optional cheap polynomial/endpoint fitter baseline if <1 day.

**Done-when:** Seq baseline in Table `tab:sota-main` (or new baselines table) with ms/batch and params.

#### Task B3: Screened heavy models documentation

**Files:**
- Modify: `denoise-opt-meta/paper/v5/subsections/related_work.tex`
- Modify: `denoise-opt-meta/paper/v5/subsections/experiments.tex`
- Modify: `denoise-opt-meta/paper/v5/subsections/limitations.tex`

- [ ] **Step 1:** Explicit paragraph: Demucs/DiffWave screened for domain mismatch (full-band speech/music denoisers vs $L{=}256$ periodic seam), compute, and OA/eval protocol mismatch.
- [ ] **Step 2:** Point to new N2N/seq baselines as the appropriate modern learned controls.

**Acceptance criteria (Phase B):**
- [ ] Primary corrupt→corrupt N2N baseline trained without meta-search leakage and without holdout tile leakage.
- [ ] Secondary sibling-supervised ceiling reported and labeled as such.
- [ ] LSTM or small 1D CNN seq baseline reported.
- [ ] Classical DualCosine/FIR retained.
- [ ] Demucs/DiffWave screening stated in TeX.
- [ ] Hard-strata numbers include new baselines.

**Risks:** Training instability; keep architectures tiny. Do not silently use favorite init weights.

---

# Phase C — Evaluation realism (wavetable-native)

**Effort:** M–L  
**Dependencies:** Sample scope locked (both); Phase A metric helpers.  
**Answers:** W1, W2 (partial), S1 (partial). **Rejects S2** (no LibriSpeech/MUSDB).

### Intent

**Locked (19 Jul grill):** **BOTH** realism tracks. **Primary:** ReelSynth-exported periods (same engine). **Secondary:** license-clean OA instrument / WT one-shots under the same wrap discontinuity protocol. Speech/music LibriSpeech/MUSDB remain OFF (not used).

### Tasks

#### Task C1: Wrap protocol for real cycles

**Files:**
- Create: `reelsynth/scripts/real_wt_wrap_protocol.py`
- Modify: `denoise-opt-meta/paper/v5/EVAL_PROTOCOL.md`

Protocol (lock in docs):
1. Load mono cycle or extract one period ($L{=}256$ resample/crop with documented rule).
2. Create ideal = periodized / endpoint-matched reference (or original closed seam).
3. Apply open-wrap cliff of amplitude $\pm\mathcal{U}(0.08,0.43)$ over `SEAM_W=8` (same as synthetic).
4. Score methods with prolonged $R$, SNR/SDR, wrap-jump, edge RMSE.

- [ ] **Step 1:** Implement loader + cliff apply + score.
- [ ] **Step 2:** Unit test on one synthetic cycle fed through real loader path (bit-comparable cliff).

#### Task C2: ReelSynth / factory export corpus (PRIMARY realism)

**Files:**
- Create: `reelsynth/src/bin/export_reelsynth_wt_cycles.rs` (or extend export bin)
- Create: `brand/artifacts/real_wt_cycles/` or `artifacts/real_wt_cycles/` (license README)
- Create: `denoise-opt-meta/paper/v5/figures/real_wt_matrix.json`

- [ ] **Step 1:** Export ≥20 cycles from factory banks / patches (saw morph, lead stacks, etc.).
- [ ] **Step 2:** Run wrap protocol + method matrix (identity, DualCosine, FIR, favorite, N2N, seq).
- [ ] **Step 3:** Add Results subsection “Wavetable-native transfer” as **primary realism** track (synthetic remains primary *controlled* corpus).

#### Task C3: OA instrument / WT one-shot pack (SECONDARY realism)

**Files:**
- Create: `denoise-opt-meta/paper/v5/SAMPLE_LICENSES.md`
- Same scoring pipeline as C2

- [ ] **Step 1:** Pick license-clean OA pack; record license + URL in SAMPLE_LICENSES.md.
- [ ] **Step 2:** Import only license-clean one-shots; no scraped commercial libs.
- [ ] **Step 3:** Score and report as **secondary** realism table under same wrap protocol.

#### Task C4: Expand Rust sound_bench if needed

**Files:**
- Modify: `reelsynth/src/sound_bench.rs`
- Modify: `reelsynth/src/bin/export_sound_bench_tiles.rs`

- [ ] **Step 1:** Only if real-cycle coverage gaps remain: add families that mimic instrument-like spectra while staying procedural.
- [ ] **Step 2:** Re-export ≥20 tiles; refresh Rust transfer table.

#### Task C5: Explicitly keep speech OOD OFF

**Files:**
- Modify: `limitations.tex`, `EVAL_PROTOCOL.md`

- [ ] **Step 1:** State LibriSpeech/MUSDB not used (locked); optional future OOD probe would be labeled and non-claim-bearing.

**Acceptance criteria (Phase C):**
- [ ] ≥20 ReelSynth-exported periods scored under wrap protocol (primary realism).
- [ ] OA instrument/WT one-shots scored under same protocol (secondary realism).
- [ ] License file present for any external samples.
- [ ] No LibriSpeech/MUSDB tables.
- [ ] Results distinguish synthetic controlled primary vs wavetable-native realism tracks.

**Risks:** Period extraction from one-shots is ambiguous; document cropping/resample rules tightly. License mistakes are blocking.

---

# Phase D — Theory cleanup

**Effort:** S  
**Dependencies:** None (TeX-first). Can parallel Phase A.  
**Answers:** W4–W6, S4 (partial), S5.  
**Locked (19 Jul grill):** DELETE Lemma 1 + trivial Props theater; keep formal $R$; explicit no wrap-closure / search-convergence guarantees.

### Intent

Delete trivial Props 1–3 theater (fold any useful one-liners into the $R$ definition). **Delete Lemma 1.** State explicitly: no wrap-closure or search-convergence guarantees. Optional honest complexity/budget note only if non-fake. Epistemic honesty over decorative theory.

### Tasks

#### Task D1: Demote / delete Props 1–3

**Files:**
- Modify: `denoise-opt-meta/paper/v5/subsections/methods.tex`
- Optional create: `denoise-opt-meta/paper/v5/subsections/appendix_immediate.tex` (if venue allows appendix; prefer deletion over appendix theater)

- [ ] **Step 1:** Remove Prop 1–2 environments; replace with at most a short “Immediate consequences of Eq. (R)” paragraph if needed for readability (or delete entirely).
- [ ] **Step 2:** Fold Prop 3 into the definition of $R$ as one sentence (“$R$ is strictly decreasing in residual RMS when pre-clamp scores lie in (0,1)”) or omit.
- [ ] **Step 3:** Grep for `\ref{prop:` and fix dangling refs.

#### Task D2: Delete Lemma 1 (locked)

**Files:**
- Modify: `methods.tex`, `discussion.tex` / `limitations.tex`

**Locked path:** delete Lemma 1; add:

> We do not claim convergence guarantees for the hybrid GA+PPO+PBT outer loop, nor a general wrap-closure theorem for arbitrary bake cells. Empirical gains are reported under the frozen protocol.

- [x] **Step 1:** User confirms delete vs prove → **DELETE** (19 Jul grill).
- [ ] **Step 2:** Apply delete path; remove “sufficient sketch” language entirely; keep formal definition of residual $R$.

#### Task D3: Optional honest addition

**Files:**
- Modify: `methods.tex` or `experiments.tex`

Only if true:
- Outer-loop complexity: population size × proposals × fit steps × batch cost (big-O style accounting), **or**
- Empirical landscape note: reward variance / plateau observations from overnight log (no fake convexity claims).

- [ ] **Step 1:** Add ≤1 paragraph; cite overnight freeze numbers already in Results.
- [ ] **Step 2:** Reject any “converges to global optimum” wording.

**Acceptance criteria (Phase D):**
- [ ] No trivial proposition theater in main Methods.
- [ ] Lemma 1 removed with explicit no-guarantee / no wrap-closure sentence.
- [ ] Formal definition of residual $R$ retained.
- [ ] No invented convergence theorem.

**Risks:** Over-deleting leaves Methods looking thin; keep $R$ definition + algorithms crisp.

---

# Phase E — Writing / positioning

**Effort:** S–M  
**Dependencies:** Phase A numbers minimum; refresh after B/C.  
**Answers:** W3 (reposition), S1 framing, Related Work N2N contrast.

### Intent

Sharper Related Work vs Noise2Noise audio restorers; soften residual overclaim; emphasize DSP/audio-engineering venue fit; update abstract/discussion with hard-cliff results.

### Tasks

#### Task E1: Related Work contrast

**Files:**
- Modify: `denoise-opt-meta/paper/v5/subsections/related_work.tex`

- [ ] **Step 1:** Paragraph contrasting: prior N2N speech restorers operate on broadband noisy speech; we operate on periodic $L$-sample seams with procedural siblings; previously we cited N2N as ranking philosophy only; Phase B now adds a same-domain N2N-style control.
- [ ] **Step 2:** OA-only cite check.

#### Task E2: Soften overclaim + venue sentence

**Files:**
- Modify: `introduction.tex`, `discussion.tex`, `conclusion.tex`, `main.tex`

- [ ] **Step 1:** Ensure every bold claim is scoped to cycle-local seam repair.
- [ ] **Step 2:** Add venue-fit sentence: contribution aimed at DSP / synthesis artifact repair (DAFx/AES), not general speech enhancement leaderboards.
- [ ] **Step 3:** Remove any residual “meta-learning SOTA” flavor without evidence.

#### Task E3: Abstract / discussion refresh after measurements

**Files:**
- Modify: `main.tex`, `discussion.tex`, `results.tex`

- [ ] **Step 1:** Insert hard-cliff top-10% headline numbers from Phase A.
- [ ] **Step 2:** Mention N2N/seq baselines once Phase B lands.
- [ ] **Step 3:** Mention wavetable-native secondary once Phase C lands.
- [ ] **Step 4:** Prose pass: no em dashes; no semicolon stacks; no fake PESQ.

#### Task E4: Protocol + changelog sync

**Files:**
- Modify: `EVAL_PROTOCOL.md`, `denoise-opt-meta/paper/CHANGELOG.md`, `PLAN_PROGRESS.md`
- Sync mirror tree

- [ ] **Step 1:** Document strata, new baselines, real-cycle protocol.
- [ ] **Step 2:** Changelog entry for v5.x peer-review response (user-visible honesty upgrades).

**Acceptance criteria (Phase E):**
- [ ] Related Work explains prior missing N2N compare and new baseline.
- [ ] Venue positioning explicit.
- [ ] Abstract reflects hard-cliff (and later B/C) without widening claims.
- [ ] Mirror synced; OA cites intact.

**Risks:** Writing ahead of numbers; gate abstract numeric updates on frozen JSON.

---

## Cross-phase acceptance checklist

- [ ] Narrow claim freeze intact (no speech SOTA pivot).
- [ ] Cliff strata (10%/25%) published.
- [ ] Identity-$R$ explained; hard-cliff is operative regime in prose.
- [ ] Primary corrupt→corrupt N2N + secondary sibling-supervised + seq baselines in tables (no holdout leakage).
- [ ] Wavetable-native realism: ReelSynth-exported (primary) + OA instrument/WT (secondary); no LibriSpeech/MUSDB.
- [ ] Theory cleaned (props theater deleted; Lemma 1 deleted; formal $R$ kept; no-guarantee sentence).
- [ ] OA-only cites; no PESQ-on-sine; no MUSHRA; no em-dash slop.
- [ ] Plan mirrored under `reelsynth/docs/papers/denoise_opt/v5/`.

---

## Suggested commit sequence (when executing)

1. `feat(eval): cliff-stratum bench + JSON` (A1–A2)  
2. `docs(paper): hard-cliff table + identity-R prose` (A3)  
3. `docs(paper): delete lemma/props theater; keep R + honesty` (D)  
4. `feat(baselines): N2N corrupt-corrupt + sibling ceiling + seq` (B)  
5. `feat(eval): ReelSynth + OA WT wrap protocol + matrix` (C)  
6. `docs(paper): related work, abstract, venue positioning` (E)

---

## Execution handoff

Plan complete and saved to:

- `denoise-opt-meta/paper/v5/PEER_REVIEW_IMPROVEMENT_PLAN.md`
- `reelsynth/docs/papers/denoise_opt/v5/PEER_REVIEW_IMPROVEMENT_PLAN.md` (mirror)

**Grill-locked (19 Jul 2026):** Lemma 1 DELETE; N2N full stress test (primary corrupt→corrupt); real samples BOTH (ReelSynth primary, OA secondary); venue DAFx/AES/arXiv DSP; grill frame = what is weak.

**Recommended first phase:** Phase A (metric & baseline honesty).

**Two execution options when ready:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — execute with `superpowers:executing-plans`, batch with checkpoints

Do not start Phase A–E implementation until the user confirms phase start. Open items remaining: seam-local metric choice and exact venue submission timing.
