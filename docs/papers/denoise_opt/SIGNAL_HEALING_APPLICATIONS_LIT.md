# Signal Healing / Wrap-Discontinuity Repair — Application Literature Survey

**Status:** research brief (literature map, not a claim of DenoiseOpt transfer results)  
**Date:** 2026-07-23  
**Scope:** Where *cycle-local* repair of period-boundary / wrap / seam artifacts — as in DenoiseOpt (unsupervised bake Θ toward an ideal sibling, scored by prolonged residual \(R\)) — could plausibly transfer.  
**Method:** web + academic search (arXiv, IEEE/ACM/AES, SIGGRAPH, biomedical venues) + MCP `research_search_papers` / `research_get_paper`. Citations verified against OA abstracts/PDFs or publisher metadata; items marked *speculative* are analogies only.

**Canonical companion paper repo:** [reeldemo/denoise-opt-meta](https://github.com/reeldemo/denoise-opt-meta)

---

## 1. Executive summary

DenoiseOpt’s core mathematical object is a **single (quasi-)period 1D cycle** with a **wrap cliff** at the period seam. The task is to find a repair operator Θ that restores continuity under prolonged tiling, while preserving spectral / musical content — scored by residual \(R\) against an ideal sibling, not by general speech-enhancement metrics.

**Best transfer domains (high):**

1. **Wavetable / virtual-analog / sample looping** — home domain; classical periodize + fade is mature; *learned* seam bake is still sparse; LoopGen and DWTS are the closest published neighbors.
2. **Granular / concatenative microsound** — grain edges are local wrap problems; classical windowing dominates; higher-order continuity repair (Lagrange, etc.) exists; NAS-over-repair-graphs is open.
3. **PSOLA / pitch-period splicing** — explicit period markers + overlap-add seams; distortion literature is large; learned *period-local* healers are underexplored vs full speech SE.
4. **Computer graphics: seamless texture / video loops / closed curves** — strongest *optimization* analogy (min-cut seams, cyclification, residual discontinuity post-process); different signal class.
5. **ECG / quasi-periodic biomedical cycles** — Cycle-GAN / diffusion “restoration” and beat-morphology priors; seams exist but clinical constraints dominate; transfer needs careful metric redesign.

**Medium:** FFT periodization / circular-convolution edge effects; rotating-machinery order tracking; robotics periodic gait continuity.

**Speculative / weak force-fits:** climate seasonality (nonstationary harmonics, not wrap cliffs); cyclic voltammetry iR-drop (not a period seam); seam carving (retargeting, not cycle wrap); generic Cycle-GAN for audio (domain translation ≠ wrap protocol).

**Prior-art gap:** Classical (a) periodize/fade/BLEP is abundant. Learning-based (b) *wrap-aware* methods exist mainly as LoopGen (inference circular attention) and DWTS (hard continuity constraint on tables). **NAS / meta-search over repair graphs scored by prolonged residual \(R\)** — DenoiseOpt’s distinctive (c) — appears largely **unoccupied** outside the DenoiseOpt line of work.

---

## 2. Problem framing (shared structure)

| Ingredient | DenoiseOpt | Adjacent fields |
|------------|------------|-----------------|
| Object | Single-period wavetable / cycle \(x[0..L)\) | Grain, pitch period, texture tile, ECG beat, shaft revolution |
| Artifact | Wrap discontinuity / cliff at \(L\!\leftrightarrow\!0\) | Click, splice buzz, spectral leakage, order-track jump, texture seam |
| Protocol | Periodize / tile \(N\) times; measure prolonged residual | Loop playback, circular DFT, closed curve, endless video texture |
| Repair Θ | Bake / denoise graph (classical ops + tiny nets) | Crossfade, window, BLEP, min-cut, morph, GAN restore |
| Score | Prolonged \(R\) vs ideal sibling + soft spectral gate | Audibility, MSE, clinical morphology, visual seam cost |

**Taxonomy used below**

| Class | Meaning |
|-------|---------|
| **(a) Classical** | Hand-designed periodize, fade, window, BLEP/BLAMP, min-cut, apodization |
| **(b) Learning-based seam repair** | Trained or inference-modified models that explicitly target wrap/seam continuity |
| **(c) NAS / meta-search over repair graphs** | Search Θ in an operator graph, scored by prolonged residual-like objectives (DenoiseOpt-like) |

---

## 3. Domain table

| Domain | Analogy strength | Key refs (see §7) | Transfer notes |
|--------|------------------|-------------------|----------------|
| Wavetable / VA / synth tables | **High** (home) | Stilson & Smith 1996; Esqueda et al. 2016; Horner et al. / Massie WT101; US6084170; Shan et al. 2021 (DWTS); Marincione et al. 2025 (LoopGen) | Direct problem match. Classical (a) strong. (b) emerging. (c) DenoiseOpt niche. |
| Sample looping & beat slicing | **High** | Massie WT101; Creative US6084170; Vorbis crosslap; DSP lore (zero-cross + equal-power XF) | Multi-period / tempo loops vs single-cycle tables; still wrap-local. |
| Granular / concatenative | **High** | Truax 1988; Roads (CMJ / microsound tradition); DAFx grain Lagrange 2021 | Grain envelopes ≈ local fade; learned Θ rare. |
| PSOLA / vocoders | **High–medium** | Charpentier & Stella 1986; Kortekaas & Kohlrausch 1997; Longster 2003; pitch-marking EUSIPCO 2002 | Period markers make seam location explicit; buzzyness ≠ wrap cliff alone. |
| FFT / circular convolution | **Medium–high** (math) | Standard DSP (Harris windows; Oppenheim & Schafer lineage); DST book leakage notes | Same wrap assumption; repair is usually *window*, not waveform heal. |
| Graphics: texture / video / closed curves | **High** (opt analogy) | Efros & Freeman 2001; Kwatra et al. 2003; Schödl et al. 2000; Zhou et al. 2013 | Min-cut / cyclify / residual stitch; 2D–3D, visual cost. |
| ECG / biomedical cycles | **Medium** | Kıranyaz et al. 2022; BeatDiff / PulseDiff (2023–2024); template priors | Quasi-period + morphology priors; clinical “don’t invent beats.” |
| Vibration / order tracking | **Medium** | Fyfe & Munck 1997; Bossio et al. 2006 (COT discontinuities) | Angle-domain wrap; discontinuities from tach resampling, not musical cliff. |
| Robotics / periodic gait | **Medium–low** | CPG + BO / RL (e.g. 2024–2025 CPG papers); smoothstep C1 joins | Continuity of *control trajectories*; different sensors/actuators. |
| Ocean / climate seasonality | **Low–medium** | Pezzulli et al. 2005; EHA / S_TIDE / F_TIDE line | Nonstationary harmonics; “periodization” usually statistical, not wrap bake. |
| NMR / spectroscopy | **Low–medium** | Standard FID apodization literature | Truncation → sinc wiggles; windowing (a), not sibling residual search. |
| Cyclic voltammetry | **Low** (force-fit) | Elgrishi et al. 2018; iR-drop / PF compensation reviews | Distortion of sweeps, not cycle-seam repair. |
| Seam carving | **Low** (bad analogy) | Avidan & Shamir 2007 | Energy paths for *retargeting*, not period wrap. |
| Generic Cycle-GAN audio | **Caution** | SEGAN / CycleGAN-VC family | Domain map ≠ wrap protocol; ECG Cycle-GAN is closer in *restoration* language only. |

---

## 4. Domain clusters (depth)

### 4.1 Wavetable / synthesizer / virtual analog — **HIGH**

**Why the analogy holds.** Single-cycle tables are *defined* by looping. A value or derivative mismatch at \(L\!\leftrightarrow\!0\) creates a click that aliases under pitch shifting — exactly DenoiseOpt’s wrap cliff. Crossfading adjacent wavetables also fails if tables are not phase-locked (Massie, “Wavetable Synthesis 101”).

**Classical (a).**  
- Alias-free discontinuity handling: BLIT / BLEP lineage (Stilson & Smith, ICMC 1996); BLAMP for corners (Esqueda et al., ISMRA/DAFx-adjacent 2016).  
- Loop construction: windowed single-period extraction + complementary fade for phase-locked tables (Massie WT101).  
- Harmonic amplitude/phase progressive matching across the loop (Creative Technology, US Patent 6,084,170, “Optimal looping for wavetable synthesis”).  
- Cosine looping-waveform transforms for smooth joins (Winbond US5,808,222).

**Learning (b).**  
- **Differentiable Wavetable Synthesis (DWTS)** (Shan et al., ICASSP 2022 / arXiv:2111.10003): learns one-period tables; **appends \(w[L]=w[0]\)** to prevent discontinuity — hard continuity constraint, not residual meta-search.  
- **LoopGen** (Marincione et al., arXiv:2504.04466, 2025): inference-time **circular padding** so a NAR music model attends across the loop seam; evaluates seam perplexity + listening tests. Closest *learned wrap protocol* for longer loops.

**NAS/meta (c).** DenoiseOpt occupies this niche for *table bake* scored by prolonged \(R\).

**Open problems.** Family-conditional hardness (nonlinear / combo cliffs); perceptual vs residual \(R\); transfer from factory AKWF tables to live-captured cycles; joint anti-alias + seam objectives.

---

### 4.2 Sample looping & beat slicing — **HIGH**

**Analogy.** Sustain loops and sliced beats are multi-period cousins of single-cycle tables: find splice points (autocorrelation / AMDF / zero-cross), then crossfade. Vorbis **crosslapping** documents how lossy encode seams become audible stairsteps without lapping (Xiph Vorbisfile crosslap docs).

**Classical (a)** dominates (equal-power XF, energy XF, FIR cyclic-prefix-style smoothing — DSP.SE folklore with solid signal-processing basis).

**(b)/(c)** mostly absent for *instrument sample* loops; LoopGen addresses *generative* music loops, not sampler library tooling.

**Differs from DenoiseOpt.** Longer loops, tempo/beat grid, stereo imaging, non-harmonic noise beds.

---

### 4.3 Granular synthesis / loop crossfading — **HIGH**

**Analogy.** Each grain is a short extracted segment; edges must not click. Envelope windows are local fades; overlap-add is a soft seam. Recent live granulation work uses **Lagrange reconstruction** across grain joins to reduce derivative discontinuities (DAFx20in21 paper 38).

**Key refs.** Truax, CMJ 12(2), 1988 (real-time granular); Roads microsound / CMJ tradition; Keller & Rolfe (window efficiency).

**Differs.** Artistic spectral coloration of windows is often *desired*; DenoiseOpt aims to restore an ideal sibling. Asynchronous clouds break strict periodization.

**Open.** Learned grain-seam graphs; residual scored on tiled grain streams.

---

### 4.4 PSOLA / vocoders — **HIGH–MEDIUM**

**Analogy.** Pitch markers define periods; TD-PSOLA duplicates/decimates windowed periods and overlap-adds. Bad marks or extreme stretch → “buzzyness,” combing, roughness (Longster PhD 2003; Kortekaas & Kohlrausch, JASA 1997).

**Classical (a).** Charpentier & Stella, ICASSP 1986 (OLA diphone); precision pitch marking (EUSIPCO 2002).

**Differs.** Speech formant preservation, F0 trajectories, concatenative unit selection — not a single static table. Full neural vocoders/SE often *bypass* period-local repair.

**Open.** Period-local healers that fix splice residual without global speech enhancement; meta-search of OLA windows / marks scored by prolonged tiled residual.

---

### 4.5 Circular convolution / FFT periodization — **MEDIUM–HIGH (mathematical)**

**Analogy.** DFT assumes the frame is one period of an infinite periodic extension. Edge mismatch → leakage (sinc sidelobes). This is the *same wrap discontinuity* in the analysis domain.

**Classical (a).** Windowing (Hann, Hamming, Kaiser…), zero-padding for linear convolution, overlap-add / overlap-save.

**Differs.** Goal is usually spectral analysis fidelity, not baking a musical cycle. Repair discards edge energy rather than inventing a continuous sibling waveform.

**Transfer idea.** Use prolonged tiled residual *plus* leakage metrics as multi-objective for Θ; or learn data-dependent apodization (still mostly (a)/(b), rare (c)).

---

### 4.6 ECG / biomedical periodic signals — **MEDIUM**

**Analogy.** Beats are quasi-periods; segmentation cuts and artifacts create discontinuities; restoration aims for morphologically valid continuous traces.

**Learning (b) — strong but different objective.**  
- **Operational Cycle-GANs for blind ECG restoration** (Kıranyaz et al., IEEE TBME 2022; arXiv:2202.00589): corrupted→clean segment mapping; removes cuts/noise while preserving arrhythmia morphology (cardiologist validation).  
- **BeatDiff / EM-BeatDiff** (NeurIPS 2024): diffusion prior on multi-lead beat morphology; inverse problems (denoise, lead reconstruct).  
- **PulseDiff** (arXiv:2310.15742): template-augmented diffusion for pulse imputation.

**Differs.** Clinical safety (“don’t invent healthy beats”), nonstationarity (HRV), multi-lead geometry. Prolonged tiling residual is *not* a clinical metric.

**Open.** Explicit beat-boundary seam scores; unsupervised repair without paired clean labels (Noise2Noise-style) with morphology constraints; **avoid** claiming Cycle-GAN ECG as wrap-protocol prior art for audio tables.

---

### 4.7 Vibration / rotating machinery — **MEDIUM**

**Analogy.** Order tracking resamples vibration into the **angle domain** (one shaft revolution ≈ one period). Computed order tracking can inject **periodic discontinuities** when angular acceleration models switch between tach pulses (Bossio et al., Shock and Vibration / related COT accuracy papers, 2006; classical COT: Fyfe & Munck 1997).

**Classical (a).** More tach pulses/rev, better interpolation, tracking filters.

**Differs.** Physical keyphasor geometry; diagnostic goal is order spectra, not musical timbre.

**Transfer.** Angle-domain “wavetable” of one revolution; residual under multi-rev tiling; meta-search of resample/repair graphs — speculative but structurally clean.

---

### 4.8 Ocean / climate / seasonal series — **LOW–MEDIUM**

**Analogy (weak).** Annual/tidal cycles; interest in *modulated* seasonality (Pezzulli, Stephenson, Hannachi, JCLI 2005; enhanced harmonic analysis / S_TIDE / F_TIDE for nonstationary tides).

**Differs.** Rarely a hard wrap cliff at a known sample index; “periodization” is harmonic modeling. Endpoint artifacts exist in some spline/IP schemes but are not musical clicks.

**Transfer.** Speculative: treat extracted seasonal cycle as a table and heal for smooth year-wrap in visualization / forcing datasets — niche.

---

### 4.9 Computer graphics: seamless textures, video loops, closed curves — **HIGH (optimization analogy)**

**Why it transfers as *search over seams*.**

| Method | Seam idea |
|--------|-----------|
| **Image quilting** (Efros & Freeman, SIGGRAPH 2001) | Min-error boundary cut in overlap |
| **Graphcut textures** (Kwatra et al., SIGGRAPH 2003) | Graph-cut optimal irregular seams; video too |
| **Video textures** (Schödl, Szeliski, Salesin, Essa, SIGGRAPH 2000) | Transition graph; **cyclify** loops; crossfade/morph to hide jumps |
| **Curvilinear pattern synthesis** (Zhou et al., CGF 2013) | Closed curves via cyclic DP + optional deformation stitch |

**Differs.** 2D/3D visual metrics; patches not 1D oscillator cycles. Seam *carving* (Avidan & Shamir 2007) is **retargeting** — cite only as contrast, not prior art for wrap repair.

**Open.** 1D audio analogue of graph-cut seam cost + prolonged residual; video-texture-style transition search for sample libraries.

---

### 4.10 Robotics / control: periodic gait trajectories — **MEDIUM–LOW**

**Analogy.** CPG oscillators produce periodic joint references; gait switches need C0/C1 continuity (smoothstep, critically damped amplitude dynamics). Online BO / RL tunes CPG parameters (e.g. arXiv:2410.16417; SYNLOCO-style CPG+RL).

**Differs.** Closed-loop stability, contact forces, sim2real — not spectral musicality.

**Transfer.** Speculative: treat one gait cycle as a multi-channel wavetable; heal wrap of joint trajectories scored by torque residual under repeated strides.

---

### 4.11 NMR / spectroscopy — **LOW–MEDIUM**

**Analogy.** Truncated FID ≡ rectangular window → sinc “wiggles” after FT — periodization/truncation artifact.

**Classical (a).** Apodization / matched filters (standard NMR processing texts).

**Differs.** Complex-valued FIDs, sensitivity–resolution tradeoff; no musical ideal sibling. Learning-based FID repair exists in broader NMR ML but is not wrap-protocol NAS.

**Cyclic voltammetry:** iR-drop distorts *I–V* loops (Elgrishi et al., J. Chem. Educ. 2018); compensation is electronic, not seam bake — **do not force**.

---

### 4.12 Analogies to handle carefully

| Phrase | Verdict |
|--------|---------|
| “CycleGAN for audio” | Usually **timbre/domain translation** (CycleGAN-VC), not wrap repair. ECG Cycle-GAN is *restoration*, still not DenoiseOpt’s Θ-search. |
| “Seam carving” | **Retargeting** energy seams ≠ period wrap. |
| “Signal healing” | Useful umbrella; in medicine often means inpainting/denoise without periodize protocol. |

---

## 5. Prior art closest to DenoiseOpt

Ranked by closeness to *wrap-aware, cycle-local repair* (not general SE):

| Rank | Work | Year | Class | Why close | Why not DenoiseOpt |
|------|------|------|-------|-----------|---------------------|
| 1 | **LoopGen** (Marincione et al.) | 2025 | (b) | Explicit loop-seam modeling; circular context; seam metric + listening | Inference hack on NAR music LM; not operator-graph NAS; not single-cycle WT |
| 2 | **DWTS** (Shan et al.) | 2021/22 | (b)/(a) | Learns wavetables; **hard** \(w[L]=w[0]\) continuity | Constraint, not residual meta-search over repair graphs |
| 3 | **Creative US6084170** optimal looping | 2000 | (a) | Progressive harmonic amp/phase matching to kill loop discontinuities | Classical DSP; no learning/NAS |
| 4 | **Massie WT101** periodize + complementary fade | 1998-ish | (a) | Phase-locked single-period extraction | Classical |
| 5 | **Video textures / graphcut / quilting** | 2000–03 | (a) opt | Search transitions / min-cut seams; cyclify | Visual domain |
| 6 | **ECG Cycle-GAN / BeatDiff** | 2022–24 | (b) | Unsupervised-ish restoration of quasi-periodic 1D | Clinical morphology; no prolonged musical \(R\) |
| 7 | **BLEP/BLAMP** | 1996–2016 | (a) | Discontinuity antialiasing for oscillators | Runtime AA, not offline table heal |
| 8 | **DAFx grain Lagrange joins** | 2021 | (a) | Higher-order continuity at grain seams | Fixed interpolator |

**Gap statement (positioning):** No surveyed work combines (i) **cycle-local wrap protocol**, (ii) **unsupervised residual vs ideal sibling under prolonged tiling**, and (iii) **hybrid GA–RL / NAS over a discrete repair-operator graph**. That intersection is DenoiseOpt’s paper claim surface.

---

## 6. Recommended next experiments & paper angles

### 6.1 Transfer experiments (ordered by ROI)

1. **AKWF / factory wavetable bake-off** — DenoiseOpt Θ vs DualCosine / US6084170-style harmonic match / DWTS continuity projection; report \(R\), edge RMSE, listening.  
2. **Sustain-loop mini-benchmark** — 0.5–2 s instrument loops; compare equal-power XF vs learned Θ; measure seam perplexity (LoopGen-style) + tiled residual.  
3. **PSOLA splice stress test** — synthetic F0 stretch; heal analysis periods only; compare to better pitch marking.  
4. **Angle-domain vibration toy** — one-rev tables with synthetic COT discontinuities; residual under multi-rev FFT orders.  
5. **1D graph-cut seam** — port Efros min-cut / Kwatra energy to 1D overlap at wrap; baseline for (a) vs DenoiseOpt (c).

### 6.2 Positioning angles

- **“Wrap protocol ≠ speech SE”** — screen Demucs/SEGAN as related-but-wrong (already in DenoiseOpt lit grounding).  
- **Bridge to LoopGen** — DenoiseOpt = *offline table/loop bake* with searchable DSP graph; LoopGen = *generative inference circularity*. Complementary, not competing.  
- **Graphics citation cluster** — quilting / graphcut / video textures as optimization ancestors for seam cost; claim 1D audio residual \(R\) as the domain-specific objective.  
- **Honesty on ECG** — cite as quasi-periodic restoration prior art; do **not** claim clinical transfer without cardiology metrics.

### 6.3 Speculative probes (low priority)

- Seasonal cycle wrap for viz; NMR learned apodization search; multi-channel gait-cycle heal.

---

## 7. Full reference list (BibTeX-ish)

```bibtex
% --- Wavetable / VA / looping ---
@inproceedings{stilson1996blit,
  title={Alias-Free Digital Synthesis of Classic Analog Waveforms},
  author={Stilson, Tim and Smith, Julius O.},
  booktitle={Proc. ICMC},
  year={1996},
  pages={332--335}
}
@inproceedings{esqueda2016blamp,
  title={Eliminating Aliasing Caused by Discontinuities Using Integrals of the Sinc Function},
  author={Esqueda, Fabi{\'a}n and V{\"a}lim{\"a}ki, Vesa and Bilbao, Stefan},
  booktitle={Proc. ISMRA / related DAFx-adjacent},
  year={2016}
  % OA PDF: ISMRA2016-48
}
@misc{massie1998wt101,
  title={Wavetable Synthesis 101, A Fundamental Perspective},
  author={Massie, Dana C.},
  howpublished={AES / industry tutorial PDF},
  year={1998},
  note={Phase-locked periodize + complementary fade}
}
@patent{creative2000loop,
  title={Optimal looping for wavetable synthesis},
  number={US6084170},
  author={Creative Technology Ltd.},
  year={2000}
}
@patent{winbond1998cosine,
  title={Method of building a database of timbre samples...},
  number={US5808222},
  author={Winbond Electronics},
  year={1998}
}
@inproceedings{shan2022dwts,
  title={Differentiable Wavetable Synthesis},
  author={Shan, Siyuan and Hantrakul, Lamtharn and Chen, Jitong and Avent, Matt and Trevelyan, David},
  booktitle={ICASSP},
  year={2022},
  note={arXiv:2111.10003}
}
@article{marincione2025loopgen,
  title={LoopGen: Training-Free Loopable Music Generation},
  author={Marincione, Davide and Strano, Giorgio and Crisostomi, Donato and Ribuoli, Roberto and Rodol{\`a}, Emanuele},
  journal={arXiv:2504.04466},
  year={2025}
}
@article{maher2005wt,
  title={Wavetable Synthesis Strategies for Mobile Devices},
  author={Maher, Robert C.},
  journal={J. Audio Eng. Soc.},
  volume={53},
  number={3},
  pages={205--213},
  year={2005}
}

% --- Granular ---
@article{truax1988granular,
  title={Real-time Granular Synthesis with a Digital Signal Processor},
  author={Truax, Barry},
  journal={Computer Music Journal},
  volume={12},
  number={2},
  pages={14--26},
  year={1988}
}
@inproceedings{dafx2021grainlagrange,
  title={Combining Zeroth and First-Order Analysis with Lagrange Polynomials to Reduce Artefacts in Live Concatenative Granulation},
  booktitle={DAFx20in21},
  year={2021},
  note={paper 38 in DAFx proceedings}
}

% --- PSOLA ---
@inproceedings{charpentier1986psola,
  title={Diphone Synthesis Using an Overlap-Add Technique for Speech Waveforms Concatenation},
  author={Charpentier, F. and Stella, M.},
  booktitle={ICASSP},
  year={1986},
  pages={2015--2018}
}
@article{kortekaas1997psola,
  title={Psychoacoustical Evaluation of the Pitch-Synchronous Overlap-and-Add Speech-Waveform Manipulation Technique...},
  author={Kortekaas, R. W. and Kohlrausch, A.},
  journal={J. Acoust. Soc. Am.},
  volume={101},
  number={4},
  pages={2202--2213},
  year={1997}
}
@phdthesis{longster2003psola,
  title={Reducing Perceived Distortion when using the TD-PSOLA Algorithm},
  author={Longster, Jennifer},
  school={Bournemouth University},
  year={2003}
}

% --- Graphics / video ---
@inproceedings{efros2001quilting,
  title={Image Quilting for Texture Synthesis and Transfer},
  author={Efros, Alexei A. and Freeman, William T.},
  booktitle={SIGGRAPH},
  year={2001},
  pages={341--346}
}
@inproceedings{kwatra2003graphcut,
  title={Graphcut Textures: Image and Video Synthesis Using Graph Cuts},
  author={Kwatra, Vivek and Sch{\"o}dl, Arno and Essa, Irfan and Turk, Greg and Bobick, Aaron},
  booktitle={SIGGRAPH},
  year={2003},
  pages={277--286}
}
@inproceedings{schodl2000videotextures,
  title={Video Textures},
  author={Sch{\"o}dl, Arno and Szeliski, Richard and Salesin, David H. and Essa, Irfan},
  booktitle={SIGGRAPH},
  year={2000},
  pages={489--498}
}
@article{zhou2013curvilinear,
  title={By-example Synthesis of Curvilinear Structured Patterns},
  author={Zhou, Shizhe and Lasram, Anas and Lefebvre, Sylvain},
  journal={Computer Graphics Forum},
  volume={32},
  number={2},
  pages={55--64},
  year={2013}
}
@article{avidan2007seamcarving,
  title={Seam Carving for Content-Aware Image Resizing},
  author={Avidan, Shai and Shamir, Ariel},
  journal={ACM TOG (SIGGRAPH)},
  year={2007},
  note={Contrast only --- retargeting, not wrap repair}
}

% --- ECG / biomedical ---
@article{kiranyaz2022ecg,
  title={Blind ECG Restoration by Operational Cycle-GANs},
  author={K{\i}ranyaz, Serkan and others},
  journal={IEEE Trans. Biomedical Engineering},
  year={2022},
  doi={10.1109/TBME.2022.3172125},
  note={arXiv:2202.00589}
}
@inproceedings{bedin2024beatdiff,
  title={Leveraging an ECG Beat Diffusion Model for Morphological Reconstruction from Indirect Signals},
  author={Bedin, Lisa and others},
  booktitle={NeurIPS},
  year={2024}
}
@article{pulsediff2023,
  title={Improving Diffusion Models for ECG Imputation with an Augmented Template Prior},
  journal={arXiv:2310.15742},
  year={2023}
}

% --- Machinery ---
@article{fyfe1997cot,
  title={Analysis of Computed Order Tracking},
  author={Fyfe, K. R. and Munck, E. D. S.},
  journal={Mechanical Systems and Signal Processing},
  volume={11},
  number={2},
  pages={187--205},
  year={1997}
}
@article{bossio2006cot,
  title={Accurate Assessment of Computed Order Tracking},
  author={Bossio, Guillermo R. and others},
  journal={Shock and Vibration},
  year={2006},
  doi={10.1155/2006/838097},
  note={Documents angle-domain discontinuities under varying acceleration}
}

% --- Climate / tides ---
@article{pezzulli2005seasonality,
  title={The Variability of Seasonality},
  author={Pezzulli, S. and Stephenson, D. B. and Hannachi, A.},
  journal={Journal of Climate},
  year={2005},
  doi={10.1175/JCLI-3256.1}
}
@article{eha2021mac,
  title={Extracting Modulated Annual Cycle in Climate and Ocean Time Series Using an Enhanced Harmonic Analysis},
  journal={Advances in Meteorology},
  year={2021},
  doi={10.1155/2021/9625795}
}

% --- Robotics (representative recent) ---
@article{cpg2024online,
  title={Online Optimization of Central Pattern Generators for Quadruped Locomotion},
  journal={arXiv:2410.16417},
  year={2024}
}

% --- NMR / CV (limited analogy) ---
@article{elgrishi2018cv,
  title={A Practical Beginner's Guide to Cyclic Voltammetry},
  author={Elgrishi, No{\'e}mie and others},
  journal={J. Chem. Educ.},
  volume={95},
  number={2},
  pages={197--206},
  year={2018},
  doi={10.1021/acs.jchemed.7b00361},
  note={iR-drop --- weak analogy only}
}

% --- DenoiseOpt method anchors (already in paper lit grounding) ---
@inproceedings{lehtinen2018n2n,
  title={Noise2Noise: Learning Image Restoration without Clean Data},
  author={Lehtinen, Jaakko and others},
  booktitle={ICML},
  year={2018}
}
@article{elsken2019nas,
  title={Neural Architecture Search: A Survey},
  author={Elsken, Thomas and Metzen, Jan Hendrik and Hutter, Frank},
  journal={JMLR},
  year={2019}
}
```

**Practical OA URLs (non-exhaustive):**  
- Stilson/Smith BLIT: https://ccrma.stanford.edu/~stilti/papers/blit.pdf  
- Esqueda BLAMP: https://www.ness.music.ed.ac.uk/wp-content/uploads/2016/12/ISMRA2016-48-1.pdf  
- DWTS: https://arxiv.org/abs/2111.10003  
- LoopGen: https://arxiv.org/abs/2504.04466  
- ECG Cycle-GAN: https://arxiv.org/abs/2202.00589  
- Image quilting: https://people.eecs.berkeley.edu/~efros/research/quilting/quilting.pdf  
- Video textures: https://www.think-cell.com/assets/en/career/talks/pdf/think-cell_article_siggraph2000.pdf  

---

## 8. Search log (transparency)

| Query theme | Primary finds |
|-------------|---------------|
| Wavetable wrap / optimal looping | Massie WT101, US6084170, Maher JAES, DWTS, LoopGen |
| Sample loop crossfade | Vorbis crosslap, DSP.SE periodize lore |
| PSOLA periodization | Charpentier 1986, Kortekaas 1997, Longster 2003 |
| Texture / video seams | Efros 2001, Kwatra 2003, Schödl 2000, Zhou 2013 |
| ECG restoration | Kıranyaz TBME 2022, BeatDiff, PulseDiff |
| Order tracking discontinuities | Bossio 2006, Fyfe 1997 |
| Seasonality | Pezzulli 2005, EHA 2021 |
| Granular artefacts | Truax 1988, DAFx Lagrange 2021 |
| NMR / CV | Apodization texts; Elgrishi CV guide (weak) |

MCP `research_search_papers` was useful for ECG / LoopGen / DWTS confirmation; broad keyword queries (“periodization”, “wrap”) were noisy (history/chem false positives) — prefer domain-qualified queries.

---

## 9. One-line takeaway

**Transfer hardest where classical fade already “good enough”; transfer most valuable where prolonged tiling exposes cliffs that fixed crossfades smear rather than heal — and where a searchable repair graph can be scored by a residual like DenoiseOpt’s \(R\).** Top soil: wavetable/sample loops, grains/PSOLA splices, and 1D analogues of graphics seam optimization.
