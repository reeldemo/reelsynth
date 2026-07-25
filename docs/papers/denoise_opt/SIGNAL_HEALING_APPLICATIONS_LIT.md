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

**Medium:** FFT periodization / circular-convolution edge effects; rotating-machinery order tracking (**promoted — see §10**); robotics periodic gait continuity; synchrophasor windows; NMR apodization.

**Speculative / weak force-fits:** climate seasonality (nonstationary harmonics, not wrap cliffs — though year-boundary spline artifacts exist, §10); cyclic voltammetry iR-drop (not a period seam); seam carving (retargeting, not cycle wrap); generic Cycle-GAN for audio (domain translation ≠ wrap protocol); radar *range* PRI ambiguity (aliasing ≠ seam heal).

**Sci/eng deep-dive:** §10 ranks natural-science and engineering transfers with verified citations and concrete non-audio experiments. **Public datasets for pilots:** §11 (+ companion [`SIGNAL_HEALING_DATASETS.md`](SIGNAL_HEALING_DATASETS.md)).

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
| Vibration / order tracking | **Medium–high** | Fyfe & Munck 1997; Saavedra & Rodríguez 2005/06 (COT accuracy); Guo et al. 2014 (envelope deformation) | Angle-domain wrap; discontinuities from tach resampling / constant-accel assumption. |
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

**Analogy.** Order tracking resamples vibration into the **angle domain** (one shaft revolution ≈ one period). Computed order tracking accuracy depends on tach sampling, interpolation, and pulses/rev; the classical constant-acceleration assumption between tach pulses is a known error source (Fyfe & Munck, MSSP 1997; Saavedra & Rodríguez, Shock and Vibration 2005/06, doi:10.1155/2006/838097). Envelope analysis after COT can further **deform** the angular-domain envelope (Cheng et al., MSSP 2014).

**Classical (a).** More tach pulses/rev, better interpolation, Vold–Kalman / hybrid COT (Bossley et al., MSSP 1999), tracking filters.

**Differs.** Physical keyphasor geometry; diagnostic goal is order spectra / bearing fault rates, not musical timbre.

**Transfer.** Angle-domain “wavetable” of one revolution; residual under multi-rev tiling; meta-search of resample/repair graphs — **structurally one of the strongest non-audio matches** (see §10).

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
@article{saavedra2006cot,
  title={Accurate Assessment of Computed Order Tracking},
  author={Saavedra, P. N. and Rodr{\'i}guez, C. G.},
  journal={Shock and Vibration},
  year={2005},
  volume={13},
  number={1},
  pages={13--32},
  doi={10.1155/2006/838097},
  note={COT accuracy vs tach sampling, interpolation, pulses/rev; OA PDF available}
}
@article{bossley1999hybridcot,
  title={Hybrid Computed Order Tracking},
  author={Bossley, K. M. and McKendrick, R. J. and Harris, C. J. and Mercer, C.},
  journal={Mechanical Systems and Signal Processing},
  volume={13},
  number={4},
  pages={627--641},
  year={1999},
  doi={10.1006/mssp.1999.1225}
}
@article{cheng2014envelopecot,
  title={Envelope deformation in computed order tracking and error in order analysis},
  author={Cheng, Weidong and Gao, Robert X. and Wang, Jinjiang and Wang, Tianyang and Wen, Weigang and Li, Jianyong},
  journal={Mechanical Systems and Signal Processing},
  volume={48},
  number={1--2},
  pages={92--102},
  year={2014},
  doi={10.1016/j.ymssp.2014.03.004}
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
| Order tracking discontinuities | Fyfe & Munck 1997; Saavedra & Rodríguez 2005; Cheng et al. 2014 envelope deformation; Bossley hybrid COT 1999 |
| Seasonality | Pezzulli 2005, EHA 2021; Arguez & Applequist 2013 (year-wrap harmonics); MODIS cubic-spline periodic BC |
| Granular artefacts | Truax 1988, DAFx Lagrange 2021 |
| NMR / CV | Apodization / FID truncation (Ernst FT-NMR tradition; facility tutorials); Elgrishi CV guide (weak) |
| Synchrophasor / PQ | Harris 1978 windows; Romano EPFL thesis; IpDFT / i-IpDFT line (Frigo–Pegoraro–Toscani); IEEE C37.118 leakage |
| OFDM / CP | Cruz-Roldán et al. arXiv:2012.04527 (ISI/ICI unified; CP/CS/windowing) |
| Radar / PRI | Classical range–Doppler ambiguity (PRI wrap ≠ waveform seam heal); Doppler CPI windowing |
| CNC / CAM | Beudaert et al. 2013 corner rounding; G2/G3 toolpath smoothing literature |
| Fatigue / rainflow | ASTM E1049; Marsh et al. 2016 residue concatenation |
| ECG beat templates | Agostinelli et al. SBMM 2016; ESBMM 2020; Cycle-GAN / BeatDiff |

MCP `research_search_papers` / `research_get_paper` (user-klaut-research) used for COT, ECG, OFDM, CNC, climate normals confirmation; broad keyword queries (“periodization”, “wrap”, “PRI wrap”) remain noisy (chemRxiv / history false positives) — prefer domain-qualified queries + DOI checks.

---

## 9. One-line takeaway

**Transfer hardest where classical fade already “good enough”; transfer most valuable where prolonged tiling exposes cliffs that fixed crossfades smear rather than heal — and where a searchable repair graph can be scored by a residual like DenoiseOpt’s \(R\).** Top soil: wavetable/sample loops, grains/PSOLA splices, and 1D analogues of graphics seam optimization.

---

## 10. Natural sciences & engineering transfer

**Status:** deep-dive addendum (2026-07-23). Scope locked to **natural sciences + engineering/technology**. Music/synth/graphics entertainment are out of scope here except where a method is engineering-core (e.g. Harris DFT windows, CNC G2 continuity). Citations below were checked via OpenAlex/Crossref/Semantic Scholar/arXiv or publisher OA PDFs; **no invented papers**.

### 10.1 Mapping rule (what is / is not a DenoiseOpt wrap)

DenoiseOpt-style transfer needs **all four**:

1. A **known period / cycle length** \(L\) (beat, shaft rev, AC cycle, CPI, closed contour, year).
2. A **measurable discontinuity at the seam** \(x[L{-}1]\!\leftrightarrow\!x[0]\) (value and/or derivative cliff).
3. A **prolonged tiling / multi-period protocol** that exposes the cliff (order spectrum, multi-rev FFT, repeated gait, tiled annual cycle, repeated load block).
4. A **repair operator Θ** scored by residual \(R\) vs an ideal sibling (or physics-constrained proxy) — not only “denoise the whole recording.”

| Maps to wrap seam | Does **not** map (do not force) |
|-------------------|----------------------------------|
| Angle-domain one-rev vibration with tach resampling cliffs | Generic bearing ML classifiers without angle periodization |
| ECG/PPG **beat templates** stitched end-to-end after length normalization | Full-trace clinical denoise without beat segmentation |
| Synchrophasor / PQ **DFT window** mismatch to exact AC period (leakage) | Optimal power flow, grid OPF, cyber-PQ unrelated to cycle edges |
| NMR/FTIR **FID truncation → sinc wiggles** (rectangular periodization) | Quantum computing-with-NMR tutorials; StyleGAN “FID” metric |
| CNC **closed contour** G0/G1 corners needing G2/G3 join | General VLA / mobile-manipulation foundation models |
| Climate **annual cycle** Dec↔Jan endpoint mismatch | Paleo calendar-month redefinition alone (orbital, not sample wrap) |
| Fatigue **block repetition / rainflow residue close** | Cyclic voltammetry iR-drop (electrochemical IR, not period seam) |
| OFDM **insufficient CP / windowing** → ISI/ICI | “Cyclic prefix” as marketing synonym for fade — CP *is* a deliberate wrap protocol, but Θ is usually length/window design, not waveform bake |
| Radar **Doppler CPI window leakage** (slow-time periodize) | Range **ambiguity** (PRI modulo fold of *true range*) — aliasing, not seam heal |

### 10.2 Ranked domains

#### HIGH (strong structural match; Θ-search plausible)

| Rank | Domain | Why high | Key verified citations | Classical (a) vs learning (b) vs NAS (c) |
|------|--------|----------|------------------------|------------------------------------------|
| H1 | **Rotating machinery / COT / angle-domain vibration** | One shaft revolution is literally a period; COT builds the angular “table”; interpolation / constant-accel assumptions inject periodic errors; multi-rev order spectra = prolonged tiling. | Fyfe & Munck, *MSSP* **11**(2):187–205, 1997, doi:10.1006/mssp.1996.0056; Saavedra & Rodríguez, *Shock Vib.* **13**(1):13–32, 2005, doi:10.1155/2006/838097; Bossley et al., *MSSP* **13**(4):627–641, 1999 (hybrid COT); Cheng, Gao, Wang, Wang, Wen, Li, *MSSP* **48**(1–2):92–102, 2014, doi:10.1016/j.ymssp.2014.03.004 (envelope deformation under COT); Bonnardot, Randall, Antoni, *IJAV* 2004 (angular resampling for bearings). | (a) dominant; (b) sparse for *seam heal*; (c) open — best engineering soil for DenoiseOpt-like \(R\). |
| H2 | **CNC / CAM closed toolpaths (engineering continuity)** | Piecewise-linear G01 paths have **tangency/curvature discontinuities**; local corner rounding restores G2/G3; closed contours must meet at start/end. Prolonged “tiling” = repeated toolpath loops / contouring. | Beudaert, Lavernhe, Tournier, *Int. J. Mach. Tools Manuf.* **73**:9–16, 2013, doi:10.1016/j.ijmachtools.2013.05.008; Beudaert et al. feedrate/jerk papers same venue 2011–2012; Zhong et al. toolpath interpolation review, *Int. J. Autom. Comput.* 2019, doi:10.1007/s11633-019-1190-y. | (a) mature (Bezier/NURBS/PH); (c) meta-search of local repair graphs scored by contour error + vibration residual is open. |
| H3 | **ECG / PPG beat-cycle templates** | Beats are quasi-periods; template methods **modulate length then concatenate**; concatenation seams + morphology fidelity are explicit. Prolonged residual = multi-beat stitch error. | Agostinelli et al., Segmented-Beat Modulation Method, *Med. Eng. Phys.* **38**(6):560–568, 2016, doi:10.1016/j.medengphy.2016.03.011; Extended SBMM, *Electronics* **9**(7):1178, 2020, doi:10.3390/electronics9071178; Kıranyaz et al., Operational Cycle-GANs, *IEEE TBME* 2022 / arXiv:2202.00589; BeatDiff, NeurIPS 2024. | (a) SBMM; (b) Cycle-GAN/diffusion strong but clinical; (c) wrap-local NAS rare. |
| H4 | **Power systems: AC-cycle / synchrophasor DFT periodization** | Phasor estimation assumes nearly periodic AC; off-nominal frequency or non-integer cycles → **spectral leakage** = wrap mismatch in analysis window. Prolonged protocol = streaming PMU estimates / PQ harmonics. | Harris, *Proc. IEEE* **66**(1):51–83, 1978 (windows); Romano, EPFL thesis 2016 (DFT synchrophasors); Frigo, Pegoraro, Toscani IpDFT / Taylor–Fourier line (e.g. *Appl. Sci.* 2021 doi:10.3390/app11052261; *IEEE TIM* 2024 doi:10.1109/TIM.2024.3384553); IEEE Std C37.118.1 performance classes (P/M). | (a) windows + IpDFT; (b)/(c) data-dependent Θ rare; transfer is multi-objective (TVE, response time), not musical \(R\). |

#### MEDIUM (same math object; different success metric or weaker cliff)

| Rank | Domain | Why medium | Key verified citations | Caveat |
|------|--------|------------|------------------------|--------|
| M1 | **NMR / FTIR FID / interferogram truncation** | Truncated FID ≡ hard rectangular window → **sinc ringing** after FT (convolution theorem). Apodization forces smooth decay to zero = classical seam softener. | Ernst & Anderson FT-NMR lineage (standard texts); Harris 1978; facility/processing notes on FID truncation (e.g. Manchester NMR processing notes; U Ottawa NMR Facility blog 2007 — cite as pedagogy, not primary research). | Repair is almost always **window**, not inventing a continuous sibling FID; sensitivity–resolution tradeoff dominates. |
| M2 | **Communications: OFDM cyclic prefix / windowed OFDM** | CP makes the channel circular so one OFDM symbol is a **periodized** block; insufficient CP or bad windowing → ISI/ICI at symbol edges. | Cruz-Roldán et al., *Intersymbol and Intercarrier Interference in OFDM Systems*, arXiv:2012.04527 (unified CP/CS/window formulation). | **Careful analogy:** CP is a *designed* wrap protocol. DenoiseOpt would map to searching CP length / TX-RX windows / edge taper Θ — not baking musical tables. Do not claim OFDM invents wrap repair. |
| M3 | **Radar / sonar: Doppler CPI periodization** | Slow-time samples over a CPI are DFT’d; edge mismatch → Doppler leakage (windows again). | Classical pulse-Doppler notes (MIT LL / textbook PRF tradeoffs); Neuberger et al. arXiv:2601.09317 (range–Doppler–acceleration; modern, waveform-focused). | **Range PRI wrap** is **ambiguity/aliasing** of true range — **not** a 1D waveform seam to heal. Only CPI / matched-filter sidelobe control is analogous. |
| M4 | **Oceanography / climate / paleoclimate seasonal cycles** | Extracted annual cycle must join Dec↔Jan; unconstrained splines create **year-boundary discontinuities**; harmonic / periodic-BC splines fix them. | Arguez & Applequist, *J. Atmos. Oceanic Technol.*, 2013, doi:10.1175/JTECH-D-12-00195.1 (constrained harmonic daily normals; replaces spline with year-end issues); Pezzulli, Stephenson, Hannachi, *J. Climate* 2005; Wongsai, Wongsai, Huete, *Remote Sens.* **9**(12):1254, 2017, doi:10.3390/rs9121254 (cubic spline with annual periodic BC). | Usually **statistical seasonality**, not a hard sample cliff; DenoiseOpt \(R\) would be niche (forcing datasets / viz / downscaling). |
| M5 | **Control / robotics: periodic gait / cyclic references** | One stride is a multi-channel period; open-loop tiling needs C0/C1 joins; CPG phases define the wrap. | Shao et al., phase-guided gait, *IEEE Robot. Autom. Lett.* 2022 / arXiv:2201.00206; Freeman et al. soft-robot gait cycles, *IEEE TRO* 2025 / arXiv:2402.03617. | Stability / contact dominate; spectral \(R\) secondary. |
| M6 | **Fatigue load cycles / rainflow residue** | Variable-amplitude histories leave **open residue**; concatenation / block repetition closes hysteresis — a form of period stitch for damage counting. | ASTM E1049 (cycle counting practices); Marsh et al., *Int. J. Fatigue*, 2016, doi:10.1016/j.ijfatigue.2015.10.007 (residue processing; Amzallag et al. 1994 method). | Goal is **damage sum**, not continuous waveform sibling; analogy is residue-under-tiling, not click removal. |

#### SPECULATIVE / WEAK (mention only with caveats)

| Domain | Verdict |
|--------|---------|
| **Cyclic voltammetry** | iR-drop / uncompensated resistance distorts *I–V* loops (Elgrishi et al., *J. Chem. Educ.* 2018). **Not** a period-seam cliff. Keep as negative control. |
| **Tribology friction loops** | Closed fretting/friction hysteresis exists, but literature is physics of wear, not wrap bake. Speculative only if angle-synced friction traces show tach-like resampling seams. |
| **Generic “signal healing” medicine** | Inpainting/denoise without periodize protocol — language overlap only. |
| **Seam carving / Cycle-GAN-VC** | Already excluded in §4.12. |

### 10.3 Domain-by-domain depth (engineering focus)

#### 10.3.1 Rotating machinery (best non-audio match)

**Object.** Vibration \(v(t)\) + tach/keyphasor → angular signal \(v(\theta)\), \(\theta\in[0,2\pi)\). One rev = DenoiseOpt cycle.

**Artifact.** COT resample times from quadratic angle model between tach pulses; coarse tach rate / bad interpolation → amplitude/phase errors that **repeat every rev** and pollute order bins (Fyfe & Munck 1997; Saavedra & Rodríguez 2005). Envelope-after-COT can further warp the synchronized envelope (Cheng et al. 2014).

**Prolonged \(R\).** Tile \(N\) revolutions of a repaired one-rev table; score order-spectrum residual vs a high-tach-rate / encoder “ideal sibling,” plus optional BPFO/BPFI peak fidelity (do not erase real faults).

**Θ search space.** Interpolation kernels, tach smoothing, local seam crossfade in angle, BLEP-like anti-cliff for impulsive faults, tiny residual nets — scored by prolonged multi-rev \(R\). This is the cleanest **(c)** experiment outside audio.

#### 10.3.2 ECG / PPG

**Object.** Beat-aligned templates after R-peak (ECG) or systolic-peak (PPG) detection.

**Artifact.** Naive template average + concatenate ignores HRV → morphology error at joins; SBMM modulates TUP duration before/after median template (Agostinelli et al. 2016). Cuts/noise still produce cliffs; Cycle-GAN restoration (Kıranyaz 2022) maps corrupted→clean segments without an explicit wrap protocol.

**Maps.** Beat-boundary continuity + multi-beat tiled residual.

**Does not map.** Inventing healthy morphology for arrhythmia (clinical veto); whole-record SE without segmentation.

**Θ score.** Edge RMSE at R–R join + morphology distance (DTW / wavelet) + cardiologist-safe constraints — **not** musical \(R\).

#### 10.3.3 Power systems / PQ / synchrophasors

**Object.** Nominal 50/60 Hz periods; DFT / IpDFT windows of a few cycles.

**Artifact.** Off-nominal \(f\), harmonics, interharmonics → leakage / picket-fence (Harris 1978; Romano 2016; IpDFT literature). This is the **analysis-domain** twin of DenoiseOpt’s wrap cliff.

**Maps.** Window / taper / fractional-cycle alignment as Θ; prolonged residual = TVE / FE / RFE over streaming windows.

**Does not map.** Healing the physical voltage waveform into a “pretty sinusoid” (would destroy PQ information). Prefer **estimator** Θ, not voltage bake.

#### 10.3.4 NMR / FTIR

**Object.** Complex FID / interferogram of length \(N\).

**Artifact.** Early truncation = multiply by rect → convolve spectrum with sinc → baseline wiggles. Apodization (exponential, cosine-bell, etc.) is classical Θ.

**Maps.** Truncation cliff at end of FID (often forced to zero, not circular wrap of a musical table). Closest when zero-filling after abrupt cutoff.

**Does not map.** Learned “sibling FID” without physical decay model — easy to hallucinate peaks. Prefer search over apodization graphs scored by residual vs long-acquisition sibling (Noise2Noise-adjacent).

#### 10.3.5 Radar / sonar

**Split carefully:**

- **PRI / range ambiguity:** returns fold modulo unambiguous range — **aliasing**. Resolve with staggered PRF, not seam bake.
- **Doppler CPI FFT:** slow-time periodization + window = medium analogy (Harris again).
- **Pulse compression range sidelobes:** matched-filter design, not cycle wrap.

Only the middle bullet is DenoiseOpt-adjacent.

#### 10.3.6 Ocean / climate / paleoclimate

**Object.** Climatological annual cycle \(c(d)\), \(d=1..365/366\).

**Artifact.** Cubic spline through 12 monthly normals without periodic BC → Dec–Jan jump; Arguez & Applequist (2013) replace that with constrained harmonics; Wongsai et al. (2017) use cubic splines with **smooth periodicity** BCs for MODIS LST.

**Maps.** Year-wrap continuity of the extracted seasonal component.

**Does not map.** Nonstationary modulated seasonality as a whole (Pezzulli 2005) — harmonic model, not cliff repair.

#### 10.3.7 CNC / CAM / tribology-adjacent manufacturing

**Object.** Closed contour toolpath \(\mathbf{r}(s)\), \(s\in[0,1]\), \(\mathbf{r}(0)=\mathbf{r}(1)\).

**Artifact.** G01 corners = G0/G1 discontinuities → feedrate collapse, vibration (Beudaert et al. 2013). Local corner rounding / NURBS transitions restore G2+.

**Maps.** Spatial wrap of a closed path; prolonged residual = repeated contouring error / accelerometer residual.

**Tribology.** Only if friction/force is sampled vs crank angle with the same tach issues as §10.3.1 — otherwise weak.

#### 10.3.8 Control / robotics gaits

**Object.** One gait cycle of joint references / CPG phases.

**Artifact.** Mode switches and open-loop tile joins need C0/C1 (smoothstep, phase resets). Shao et al. (2022) use explicit phases as gait interface.

**Maps.** Multi-channel wavetable of one stride; \(R\) = torque / tracking residual under repeated strides.

**Does not map.** End-to-end RL policies without an explicit cycle table.

#### 10.3.9 Communications (OFDM) — careful

CP + optional CS + TX/RX windows exist specifically to manage **circularity and edge interference** (Cruz-Roldán et al. 2020). DenoiseOpt-like work would be **meta-search of CP/window graphs** under multipath traces scored by BER/SINR — engineering-valid but culturally different from waveform healing. State as **protocol analogy**, not prior art for audio tables.

#### 10.3.10 Materials: fatigue vs voltammetry

- **Fatigue:** Rainflow leaves residue; Marsh et al. (2016) show **concatenating residue periods** recovers transition cycles — prolonged tiling of load blocks. Medium structural analogy.
- **CV:** Keep Elgrishi 2018 as **anti-citation** (iR-drop ≠ wrap).

### 10.4 Concrete experiment ideas (outside audio)

1. **Angle-domain bearing “wavetable” bake (H1 — primary).**  
   Record vibration + high-rate encoder on a run-up. Build one-rev tables with deliberately degraded COT (1 ppr tach, linear interp). Search Θ (interp + local angle-seam ops) scored by prolonged multi-rev order residual vs encoder-ideal sibling. Report BPFO visibility before/after (must not erase faults).

2. **ECG beat-seam residual benchmark (H3).**  
   MIT-BIH / CPSC beats; compare STM vs SBMM vs DenoiseOpt-like Θ on **beat-join RMSE** + multi-beat tiled residual, with arrhythmia held out. Cardiology metrics mandatory; no claim of clinical device.

3. **Synchrophasor window NAS (H4).**  
   Synthetic off-nominal + harmonic grids (IEEE C37.118.1 test suite). Search short operator graphs over window family / IpDFT depth scored by TVE+latency Pareto — prolonged = streaming windows.

4. **Closed G01 contour heal (H2).**  
   Synthetic square/rounded CAD contours exported as dense G01. Meta-search local corner repairs under chord-error constraint; score accelerometer residual on a desktop CNC or sim vs Beudaert-style analytic blend baseline.

5. **FID apodization search (M1 — low-cost probe).**  
   Truncate long FIDs; search classical apodization graphs scored by residual vs full-length sibling spectrum (peak list + baseline wiggle energy). Positions DenoiseOpt \(R\) against matched-filter theory.

### 10.5 Sci/eng takeaway (ranked for DenoiseOpt transfer)

**Best bets:** (1) angle-domain machinery COT, (2) CNC closed-contour continuity, (3) ECG/PPG beat-template seams, (4) synchrophasor/PQ window periodization, (5) NMR FID apodization search as a cheap physics probe.  
**Handle with care:** OFDM CP (protocol twin), radar Doppler windows (yes) vs PRI range ambiguity (no), climate year-wrap (statistical), fatigue residue concat (damage, not waveform).  
**Reject as wrap prior art:** cyclic voltammetry iR-drop, generic medical “healing,” seam carving, musical LoopGen (out of this section’s scope but still the closest *audio* neighbor).

---

## 11. Public datasets for sci/eng transfer experiments

**Status:** dataset inventory addendum (2026-07-23). Companion one-pager: [`SIGNAL_HEALING_DATASETS.md`](SIGNAL_HEALING_DATASETS.md).  
**Rule:** only datasets verified via official landing pages / DOI / PhysioNet / NASA / Zenodo / Mendeley / IEEE DataPort OA. No invented collections. Licenses as stated by hosts (always re-check before redistribution).

### 11.1 How to construct wrap cliffs (shared recipes)

| Domain | Period \(L\) | Cliff construction | Ideal sibling |
|--------|--------------|--------------------|---------------|
| Bearings / COT | One shaft rev (from RPM metadata, Hall pulse, or synthetic tach) | Resample \(v(t)\!\to\!v(\theta)\) with **degraded** COT (1 ppr tach, linear interp, constant-accel assumption); seam = \(\theta=0\!\leftrightarrow\!2\pi\) | Same run with high-rate encoder / cubic interp / many ppr; or withhold last \(N\) revs as clean loop |
| CNC contour | Closed toolpath loop \(s\in[0,1]\), \(\mathbf{r}(0)=\mathbf{r}(1)\) | Dense G01 export of CAD; leave G0/G1 corners; optional resample feed along path | Analytic G2/G3 blend (Beudaert-style) or NURBS path from same CAD; accel residual on repeated loops |
| ECG / PPG | Beat between R–R (or systolic peaks) | Segment → length-normalize → **concatenate**; cliff at join | SBMM-modulated template (Agostinelli) or cardiologist-clean beat; multi-beat tiled residual |
| PQ / PMU | Nominal 50/60 Hz AC cycle or DFT window | Non-integer cycles / off-nominal \(f\) / abrupt window; leakage = analysis wrap | IpDFT / Taylor–Fourier / Hann sibling on same synthetic IEEE 1159 / C37.118.1 traces |
| NMR FID | Truncated FID length \(N\) | Hard cutoff (rect window) before FT → sinc wiggles | Full-length acquisition or exponential apodization matched to \(T_2^*\); residual vs long sibling spectrum |
| Climate year | Day-of-year \(1..365\) | Unconstrained monthly→daily spline without periodic BC | NOAA constrained harmonic normals (Arguez & Applequist lineage) |

### 11.2 Ranked public datasets

| Rank | Dataset | Domain | URL | License (host) | Size / rate / ch | Why wrap/seam | Ideal-sibling idea | Download | Citation |
|------|---------|--------|-----|----------------|------------------|---------------|--------------------|----------|----------|
| **1** | **CWRU Bearing Data Center** | Rotating / vibration | https://engineering.case.edu/bearingdatacenter | Academic open download (no formal CC on site; cite CWRU; re-check terms) | MATLAB `.mat`; **12 kHz** & **48 kHz** DE; FE @ 12 kHz; DE/FE/(BA); RPM in files; loads 0–3 HP (~1797–1720 RPM) | Constant-speed → one-rev tables; invent COT cliff via coarse tach resample | Fine interp / 48 kHz vs degraded 12 kHz COT on same fault class; do not erase BPFO/BPFI | **Easy** (direct `.mat` links) | CWRU Bearing Data Center; widely used seeded-fault benchmark |
| **2** | **Paderborn KAt Bearing DataCenter** | Rotating / vib + current | https://mb.uni-paderborn.de/en/kat/research/bearing-datacenter | **CC BY-NC 4.0** (commercial needs author OK) | Vib + motor currents @ **64 kHz**; speed/torque/load/temp @ lower rate; 4 s × 20 reps × 4 op. conditions; 32 bearing codes | Sync speed → angle domain; high SR for order spectra under multi-rev tile | High-tach / fine-interp sibling vs 1-ppr synthetic tach cliff | **Easy–medium** (uni download + cite) | Lessmeier et al., *PHM Europe* 2016, doi:10.36001/phme.2016.v3i1.1577 |
| **3** | **MFPT Fault Data Sets** | Rotating / vibration | https://www.mfpt.org/fault-data-sets/ (GitHub mirror: mathworks/RollingElementBearingFaultDiagnosis-Data) | Academic CBM use (mirror often **CC BY-NC-SA** — check mirror) | Rig: **97 656** sps (6 s) & **48 828** sps (3 s); 1-ch \(g\); shaft **25 Hz**; loads 0–300 lb + 3 real-world files | Known shaft rate → exact \(L\) samples/rev; wrap after COT | Baseline vs fault under same rate; degraded vs cubic COT | **Easy** | Bechhoefer, *Condition Based Maintenance Fault Database…*, MFPT, 2013 |
| **4** | **MIT-BIH Arrhythmia (mitdb)** | ECG beats | https://physionet.org/content/mitdb/1.0.0/ | **ODC-By 1.0** | 48 × ~30 min; **2 ch**; **360 Hz**; ~110k beat annotations | R-peak segments → length-norm → stitch; join cliff | Neighbor normal beats / SBMM; hold out VEB/SVEB | **Easy** (wget / AWS open) | Moody & Mark, *IEEE Eng. Med. Biol.* 2001; PhysioNet |
| **5** | **PTB-XL** | ECG clinical | https://physionet.org/content/ptb-xl/1.0.3/ | **CC BY 4.0** | 21 799 × 10 s; **12 leads**; **500 Hz** (also 100 Hz) | Dense beats for template banks; stitch seams | Clean sinus subset vs arrhythmia; multi-beat \(R\) | **Easy** | Wagner et al., *Sci. Data* 2020, doi:10.1038/s41597-020-0495-6 |
| **6** | **UORED-VAFCLS (Ottawa)** | Rotating / vib + acoustic + Hall | https://data.mendeley.com/datasets/y2px5tg92h | **CC BY 4.0** | 60 sets × 10 s; **42 kHz**; accel + mic + Hall speed + load + temp | Hall speed enables true angle periodization | Healthy vs developing vs faulty; fine vs coarse tach | **Easy** (Mendeley) | Sehri & Dumond, *Data in Brief* 2023, doi:10.1016/j.dib.2023.109327 |
| **7** | **NASA IMS / Cincinnati bearings** | Rotating / run-to-failure | https://phm-datasets.s3.amazonaws.com/NASA/4.+Bearings.zip · https://data.nasa.gov/dataset/ims-bearings | **U.S. Government work** (public) | 3 run-to-fail; 1 s snaps × **20 480** pts @ **20 kHz**; 4–8 ch; ~2000 RPM | Constant RPM → rev tables; prognostics cliffs over life | Early healthy snap as sibling for later degraded wrap | **Easy** (S3 zip) | Lee, Qiu, Yu, Lin & Rexnord (2007), NASA PCoE; Qiu et al., *JSV* 2006 |
| **8** | **PPG-DaLiA** | PPG (+ ECG GT) | https://archive.ics.uci.edu/dataset/495/ppg-dalia | **CC BY 4.0** | 15 subjects; wrist BVP **64 Hz**; chest ECG **700 Hz**; accel | Systolic-peak cycles; motion-corrupt seams | ECG-aligned clean beats vs motion-corrupted wrist PPG | **Easy–medium** (~2.8 GB) | Reiss et al., *Sensors* 2019, doi:10.3390/s19143079 |
| **9** | **KIT multimodal CNC milling** | CNC / G-code + sensors | https://doi.org/10.35097/hvvwn1kfwf7qt48z | **CC BY 4.0** | ~6 h; controller **500 Hz**; force/accel **10 kHz**; NC + CAD (.stp); 33 trials | Closed contours from NC/CAD; G01 corner discontinuities; loop residual = repeated toolpath vib | Analytic blend of same contour; sync force/accel on loop | **Medium** (RADAR4KIT) | Ströbel et al., 2025, doi:10.35097/hvvwn1kfwf7qt48z; *Data in Brief* / KIT |
| **10** | **Bosch CNC Machining** | CNC vib monitoring | https://github.com/boschresearch/CNC_Machining | Data **CC BY 4.0**; code BSD-3 | 3 machines × 15 ops; triax **2 kHz**; good/bad labels | Weak for *geometric* contour wrap; use as process-loop vib tile only | “Good” process cycle as sibling for “bad” | **Easy** (Git LFS/h5) | Tnani, Feil, Diepold, *Procedia CIRP* 2022, doi:10.1016/j.procir.2022.04.022 |
| **11** | **IEEE 39-bus PMU (OA DataPort)** | Synchrophasor | https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model | IEEE DataPort **Open Access** (free IEEE account) | 10 gen PMUs; ~86.6 s; **5197** frames/gen; V/I mag∠, \(f\), ROCOF | Streaming window periodization / off-nominal leakage on derived AC | IpDFT sibling; synthetic IEEE C37.118.1 waveforms preferred for raw-cycle cliffs | **Easy** (login) | RTDS + GTNET PMU; IEEE DataPort record |
| **12** | **IEEE PES ISS capacitor PQ essays** | Power quality waveforms | https://site.ieee.org/pes-iss/data-sets/ | Academic / non-commercial (PES ISS terms) | 1380 files; V/I/P lab essays under harmonic voltages | Real harmonic AC cycles; DFT window mismatch | Windowed vs IpDFT on same essay | **Easy** (zip) | Spavieri et al., *Appl. Soft Comput.* 2017, doi:10.1016/j.asoc.2017.02.017 |
| **13** | **BMRB / Metabolomics Workbench FIDs** | NMR FID | https://bmrb.io/ · e.g. MW ST000892 https://www.metabolomicsworkbench.org/ | Open academic archives (cite entry + NIH MW terms) | Bruker `fid` / nmrML; study-dependent \(N\), complex | Truncate FID → sinc; apodization Θ search | Full FID sibling; matched exponential window | **Medium** (rsync / study zip) | Ulrich et al. BMRB; MW study DOIs (e.g. 10.21228/M8D97C) |
| **14** | **NOAA U.S. Daily Climate Normals 1991–2020** | Seasonal year-wrap | https://www.ncei.noaa.gov/products/land-based-station/us-climate-normals | Public U.S. gov data | Daily T/P normals; ~15k stations; CSV/netCDF | Dec↔Jan continuity of annual cycle | Constrained harmonic normals vs naive spline | **Easy** | Arguez & Applequist, *JTECH* 2013; NCEI C01621 |
| **—** | **CV Zenodo dumps** (bonus / negative control) | Cyclic voltammetry | e.g. https://doi.org/10.5281/zenodo.11230180 | Usually CC BY | Small \(I\!-\!V\) loops | **Do not** treat as wrap cliffs (iR-drop ≠ seam) | N/A — anti-dataset | Easy | Elgrishi et al. 2018 framing; Zenodo CV deposits |

**CNC honesty note:** No widely used public set is labeled “closed toolpath seam residual.” **KIT** is the best obtainable proxy (NC + CAD + synced accel). Prefer **synthetic G01 cliffs from CAD** scored against Beudaert-style blends for a clean DenoiseOpt pilot; use KIT to show transfer to real machine signals.

**PMU honesty note:** Many DataPort PMU sets are **already phasors**. For waveform wrap cliffs, synthesize IEEE 1159 / C37.118.1 test signals; use OA PMU sets for streaming estimator Θ / TVE scores.

### 11.3 Top 3 starter packs (download tomorrow)

1. **Machinery wrap pilot (highest ROI):** CWRU (12/48 kHz) **+** MFPT (known 25 Hz shaft) **+** optional Paderborn (64 kHz, CC BY-NC). Build one-rev tables; inject COT cliffs; score multi-rev order residual vs fine-interp sibling; report BPFO/BPFI preservation.  
2. **Beat-seam biomedical pilot:** MIT-BIH (annotations, ODC-By) **+** PTB-XL (12-lead scale, CC BY) **+** optional PPG-DaLiA. Segment → normalize → stitch; score join RMSE + multi-beat tiled \(R\); arrhythmia hold-out.  
3. **Physics / manufacturing probe:** KIT CNC (NC+CAD+10 kHz sensors) **or** synthetic G01 contours **+** one NMR FID study (MW/BMRB truncate→apodize) **+** NOAA daily normals year-wrap as cheap statistical control.

### 11.4 Search log (datasets)

| Query theme | Verified finds |
|-------------|----------------|
| Bearings | CWRU, Paderborn/Lessmeier 2016, MFPT/Bechhoefer 2013, NASA IMS, UORED-VAFCLS |
| CNC | KIT multimodal (RADAR CC BY), Bosch CNC (CC BY), Zenodo i-CNC chatter (vib-only, weaker) |
| ECG/PPG | MIT-BIH, PTB-XL, PPG-DaLiA; MIMIC-Ext-PPG = credentialed (skipped for “tomorrow”) |
| PMU/PQ | IEEE 39-bus OA DataPort; PES ISS Spavieri capacitors; many PQ sets request-only → deprioritized |
| NMR | BMRB timedomain rsync; Metabolomics Workbench FID studies |
| Climate | NOAA Daily Normals 1991–2020 |
| CV | Zenodo CV deposits kept as **negative control** only |
