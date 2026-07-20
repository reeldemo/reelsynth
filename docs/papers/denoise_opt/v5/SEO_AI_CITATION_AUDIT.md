# SEO & AI-citation audit — DenoiseOpt paper v5

**Scope:** `paper/v5/` (`main.tex` + subsections). Analytical audit; not a full rewrite.  
**Date:** 19 July 2026  
**Sources applied:** paper text; `TITLES.md`; ReelSynth `brand/BRAND.md` + `brand/MAJICO.md`; Majico `seo-foundations` skill (on-page + AEO/GEO checklist); Majico blog SEO practices (primary-keyword placement, uniqueness, density guardrails, structured FAQ / quotable facts).  
**Majico MCP note:** ReelSynth project guidelines were not readable under the current OAuth session (`projectId` in `MAJICO.md` returned inaccessible). Audit uses local brand docs + Majico SEO skill / blog SEO plan patterns instead.

**Concurrent-agent status (confirmed landed):**
- Plural title in `main.tex` / `TITLES.md`: *Unsupervised Deep Audio Denoising Algorithms via…*
- Results §`Top-5 architectures (current leaderboard snapshot)` (`\label{sec:top5-arch}`) with five distinct bake graphs + latency/params

---

## Executive verdict

Scholarly keyword front-loading in the title is strong. Entity salience of the **named method (DenoiseOpt)** and **product substrate (ReelSynth)** is weak for LLM retrieval: neither appears in title, abstract, keywords, or contribution bullets. Quotable interim numbers exist in Experiments/Results but are under-signaled in the abstract. Section headings are generic IMRaD magnets, not query magnets. Bib used/screened map is a strength for citation-graph SEO; a few high-salience anchors are still missing or non-OA-only.

---

## 1. Primary / secondary keywords

### Current (observed)

| Location | Dominant phrases |
|----------|------------------|
| **Title** | unsupervised · deep audio denoising algorithms · hybrid reinforcement learning · genetic algorithm · meta-learning |
| **Keywords** | unsupervised learning · audio denoising · NAS · RL · genetic algorithms · wavetable synthesis · meta-learning |
| **Abstract** | wavetable · phase wrap · seam crackle · DualCosine · prolonged residual \(R\) · GA / PPO / PBT / MoE · unsupervised (implied via “need not be paired”) |
| **Intro** | cycle-local seam restoration · wrap discontinuity · RL+GA(+PBT) · Noise2Noise · NAS |
| **Conclusion** | wavetable wrap discontinuity · hybrid RL+GA · used/screened lit list |

### Recommended primary (pick one spine)

**Primary (academic SERP / Scholar / arXiv):**  
`unsupervised audio denoising` + `wavetable` (or `cycle-local seam restoration`)

**Secondary cluster:**
- `neural architecture search` / `NAS`
- `reinforcement learning` / `PPO`
- `genetic algorithm` / `evolutionary search`
- `meta-learning` / `hybrid RL–GA`
- `prolonged residual` / `residual-scored`
- `wrap discontinuity` / `seam crackle` / `phase wrap`
- **Named method:** `DenoiseOpt` (entity, not a stuffing keyword)
- **Implementation:** `ReelSynth` (once in abstract footer / tooling / conclusion — not every section)

### Current vs recommended placement

| Slot | Current | Recommended |
|------|---------|-------------|
| Title | Strong unsupervised + audio denoising; no DenoiseOpt (correct — TITLES.md rejects overnight tags) | Keep plural title; do **not** force DenoiseOpt into title |
| Abstract sentence 1–2 | Phenomenon-first (good) | Keep phenomenon; add explicit “We call this protocol **DenoiseOpt**” once |
| Keywords | Missing method name + wrap/seam terms | Add `DenoiseOpt`, `wrap discontinuity` or `seam restoration`; optional `proximal policy optimization` |
| Intro ¶1 | Strong problem terms | Already good; keep `cycle-local seam restoration` as definitional hook |
| Contributions | No DenoiseOpt name | Name method in contribution (1) or (2) |
| Conclusion | Good topical echo | Echo primary keyword + DenoiseOpt + one numeric claim when final |

**Density guardrail (Majico-adapted):** primary phrase in title + abstract + intro + one H2-equivalent + conclusion; avoid repeating the full title string in every paragraph (blog rule: reject >~2% primary-keyword density).

---

## 2. Entity & method salience (LLM retrieval / citation)

LLMs and answer engines prefer **stable named entities** they can cite as atomic facts.

| Entity | Salience now | Gap | Target |
|--------|--------------|-----|--------|
| **DenoiseOpt** | Appears in results/related_work/comments; **absent** from title/abstract/keywords/intro contributions | Method not retrievable as “what is DenoiseOpt?” | One formal definition sentence + keywords entry |
| **Prolonged residual \(R\)** | Strong formula + semantics | Good | Keep; add one-line plain-English synonym in abstract |
| **Cycle-local seam restoration** | Defined in intro | Not in keywords | Add to keywords or abstract |
| **DualCosine baseline** | Strong comparator | Abstract mentions DualCosine ops but not as baseline | Abstract: “vs DualCosine baseline \(R\approx\ldots\)” when numbers freeze |
| **Hybrid outer loop (GA+PPO+PBT+MoE)** | Clear in abstract | Acronym soup without expansion on first abstract hit of PPO | Spell out once: “proximal policy optimization (PPO)” |
| **ReelSynth** | URL only | Product/engine name never stated | One noun phrase: “ReelSynth wavetable engine” next to the GitHub link |
| **SOTA claims** | Correctly **avoided** for speech/music SOTA | Good for honesty; weak for “citable SOTA” bait | Prefer **protocol-SOTA on this geometry** phrasing after budget completes — never generic SOTA |

### Atomic facts LLMs can already quote (keep / harden)

1. \(R\in[0,1]\), \(1=\)best, prolonged tiled residual vs no-cliff ideal.  
2. Lit-combo CPU study (separate protocol): `evo_explore_515` \(R\approx 0.824\) vs DualCosine \(\approx 0.698\) (\(\Delta\approx +0.126\)).  
3. Interim favorite: `champion_iter_000235`, \(R\approx 0.991\), \(\approx 37\)k params, \(\approx 3.15\) ms/batch.  
4. Classical vs AI: DualCosine \(R\approx 0.820\); seam_fir3 \(R\approx 0.963\); neural favorite \(\Delta R\approx +0.171\) vs DualCosine.  
5. Family hardness: `nonlinear` / `combo` weakest on 100k-cycle bench.

Mark each as **interim** vs **frozen** in a single takeaway box so models do not over-claim.

---

## 3. Abstract optimization

### Strengths
- Unique problem geometry (wrap cliff / seam crackle) — high differentiation vs generic speech denoise abstracts.
- Method stack named (GA, PPO, PBT, MoE).
- Honest “experiments ongoing” framing.

### Weaknesses (Majico AEO lens)
| Issue | Detail |
|-------|--------|
| No named method | Cannot answer “What is DenoiseOpt?” from abstract alone |
| Soft uniqueness | Ends on process (“draft fixes…”, “tables wait”) instead of a quotable result |
| Number sparsity | Almost no numeric claims; Results already have cite-worthy deltas |
| Acronym dump | PPO/PBT/MoE without one expansion |
| Unsupervised signal | Implied, not lexicalized in first 2 sentences (title carries it) |

### Recommended abstract shape (structure only)

1. **Problem** (1–2 sentences) — keep current wrap/seam opening.  
2. **Name + objective** — “We introduce **DenoiseOpt**, a residual-scored hybrid RL+GA meta-search for cycle-local seam restoration…”  
3. **How judged** — \(R\in[0,1]\), unsupervised / no paired clean required.  
4. **One interim or frozen numeric claim** — e.g. favorite \(R\approx 0.991\) vs DualCosine \(\approx 0.82\) on matched protocol (**label interim** until budget done).  
5. **Code** — already above abstract; optional “implemented in ReelSynth” once.

Do **not** stuff Reeldemo/Majico marketing language into the abstract.

---

## 4. Section headings as query magnets

| Current | Query-magnet alternative (P1) |
|---------|-------------------------------|
| Introduction | Keep; optional subtitle not in LaTeX `\section` |
| Related Work | Keep; subsection titles are already good magnets |
| Methods | → `Methods: Prolonged Residual and Hybrid RL–GA Search` |
| Experiments | → `Experiments: Overnight PPO+GA+PBT+NAS Campaign` |
| Results | Keep; **Top-5 architectures** already excellent |
| Discussion | → `Discussion: Family-Conditional Hardness and Plateaus` |
| Tooling and AI use | Keep (transparency / trust for AI-citation era) |
| Limitations | Keep |
| Outlook: follow-up publication | Keep (good AEO “next question” bait) |
| Conclusion | Keep |

Related-work subsections (`Label-free restoration`, `Evolutionary reinforcement learning hybrids`, `Used versus screened`) are already strong SERP/LLM headings — preserve.

---

## 5. Bib / related work — citation-graph SEO

### Strengths
- Explicit **used vs screened** protocol is rare and citable.  
- OA arXiv links on many anchors (Noise2Noise, PPO, PBT, ERL, ENAS, MoE, DDSP, Wave-U-Net).  
- Honest non-OA flags for VA/BLEP lineage.

### Gaps (completeness for scholarship SEO)

| Gap | Why it matters | Suggestion |
|-----|----------------|------------|
| BLEP / bandlimited step as named cite | Domain readers search “BLEP wavetable”; text mentions BLEP without a dedicated bib key | Add classic Stilson/Smith or Välimäki BLEP cite if OA/available |
| Speech enhancement survey / metric paper | Positions against PESQ/SI-SDR world without citing it | One screened survey (e.g. recent speech enhancement review) clarifies non-claim |
| Conv-TasNet OA | Bib marks non-OA; arXiv often exists | Prefer arXiv URL if available for link SEO |
| SEGAN screened | Present | OK |
| Self-cite path | No prior DenoiseOpt / ReelSynth paper DOI yet | On arXiv release: cite GitHub + this preprint consistently |

**Do not** inflate bib with unrelated high-cite papers solely for PageRank — Majico uniqueness rule applies: every cite must earn its used/screened slot.

---

## 6. AI citation bait checklist

Majico AEO/GEO pattern → academic adaptation:

| Pattern | Status | Action |
|---------|--------|--------|
| Clear definition of named method | Weak | Add DenoiseOpt definition (intro + abstract) |
| Quotable claim with numbers | Strong in body; weak in abstract | Promote 1–2 deltas to abstract when frozen |
| FAQ / Q–A block | Absent | Optional `\paragraph{Takeaways.}` or short FAQ in Discussion (2–4 Qs) |
| Tables LLMs quote | Figures yes; formal tables sparse | Add Table: Top-5 \(R\) / latency / params; Table: Classical vs AI |
| Boxed takeaways | Absent | One `\paragraph{Key numbers (interim).}` in Results |
| Reproducible identifiers | Good (arch names, GitHub URLs, ORCID) | Keep; add commit/tag when freezing |
| Negative space / non-claims | Excellent | Keep “not SOTA on speech/music” — models will cite this as scope |

### Suggested FAQ bait (P1, Discussion or end of Results)

1. What artifact does DenoiseOpt denoise? → Wavetable wrap / cycle-local seam crackle.  
2. What is \(R\)? → Prolonged tiled residual vs no-cliff ideal, \(1=\)best.  
3. Is clean audio required? → No paired studio-clean cycles for every trial.  
4. How does it compare to DualCosine? → Cite frozen \(\Delta R\) on matched protocol.

---

## 7. Majico / brand alignment (dual academic + product)

ReelSynth brand voice (`BRAND.md`): precise, technical, no hype, verb-forward; instrument not agent.

| Token | Academic paper policy | Rationale |
|-------|----------------------|-----------|
| **DenoiseOpt** | **Yes** — method name in abstract, keywords, intro, methods, conclusion | Entity salience; not spam if once-per-section |
| **ReelSynth** | **Yes, sparse** — code line + tooling + conclusion (“engine”) | Product discoverability without looking like an ad |
| **wavetable / wrap / crackle / seam** | **Yes** — core technical vocabulary | Matches brand + science |
| **Reeldemo** | **No** in abstract/title; optional footnote “from Reeldemo org” | Avoid commercial bleed |
| **Majico** | **Never** in paper body | Internal tooling brand; tooling section already says Klaut/Ollama |
| “killing wrap crackle”, “SOTA seam bake” | **Never** | TITLES.md + brand no-hype |

**Dual discoverability pattern:** Scholar/arXiv query → unsupervised audio denoising + wavetable; GitHub/LLM query → DenoiseOpt + ReelSynth. Title serves the first; abstract/keywords/URLs serve the second.

---

## 8. Prioritized action list

### P0 — high ROI, low risk (do now)

| # | Edit | Exact phrase suggestion |
|---|------|-------------------------|
| P0.1 | Keywords | Append: `DenoiseOpt $\cdot$ wrap discontinuity` (or `seam restoration`) |
| P0.2 | Abstract | After problem sentences, insert: `We call the residual-scored hybrid search \textbf{DenoiseOpt}.` |
| P0.3 | Abstract | Expand once: `proximal policy optimization (PPO)` on first PPO mention |
| P0.4 | Code blurb | `Implemented in the ReelSynth engine:` before the reelsynth URL |

### P1 — after overnight freeze / next prose pass

| # | Edit | Exact phrase suggestion |
|---|------|-------------------------|
| P1.1 | Abstract closing | Replace process-hedge with: `On a matched CUDA inference bench, a compact favorite reaches \(R\approx 0.991\) versus DualCosine \(R\approx 0.82\) (interim; final mean-\(R\) after declared budget).` — only if numbers remain valid |
| P1.2 | Contribution (2) | `Hybrid DenoiseOpt outer loop.` instead of only `Hybrid meta-outer loop.` |
| P1.3 | Methods heading | `Methods: Prolonged Residual Objective and DenoiseOpt Search` |
| P1.4 | Results | Add `booktabs` **Table** mirroring Top-5 + Classical vs AI (LLM-quotable) |
| P1.5 | Discussion | `\paragraph{Takeaways.}` with 3 bullets + optional 3 FAQ questions |
| P1.6 | Conclusion | Open with: `DenoiseOpt frames wavetable wrap discontinuity as unsupervised audio denoising…` |

### P2 — scholarship / SEO polish

| # | Edit |
|---|------|
| P2.1 | Add BLEP primary citation; prefer OA URLs for Luo2019 if available |
| P2.2 | One screened speech-enhancement survey for explicit non-claim boundary |
| P2.3 | arXiv categories + comments line: `audio denoising, wavetable, NAS, RL` |
| P2.4 | Align `plan.md` / `klaut_meta.json` titles with plural academic title (stale “Overnight” titles hurt internal consistency) |
| P2.5 | When DOI/arXiv ID exists, add citation snippet to both GitHub READMEs |

---

## Concurrent-agent checklist

| Item | Status |
|------|--------|
| Plural title (*Algorithms*) | **Landed** (`main.tex`, `TITLES.md`, commit `32eb3f5`) |
| Top-5 architectures section | **Landed** (`results.tex` `\ref{sec:top5-arch}`) |
| SEO audit doc | **This file** |
| Abstract/keywords DenoiseOpt salience | **Gap** → P0 recommendations (optional micro-edit applied in same change set if noted below) |

---

## Optional P0 micro-edits applied?

**Yes (this change set):** P0.1 keywords (`DenoiseOpt`, `wrap discontinuity`); P0.2 abstract naming sentence; P0.3 PPO expansion; P0.4 code blurb (`ReelSynth engine + DenoiseOpt meta-search`). No body-section rewrite.

## P1 status (5k gate, 2026-07-19)
- Abstract now includes frozen matched-bench claim: favorite \(R\approx 0.991\) vs DualCosine \(R\approx 0.820\), plus overnight 5k-gate champion \(R\approx 0.9909\) vs DualCosine \(\approx 0.8166\).
- Bibliography is **OA-only** (39 entries, 39 PDFs downloaded).
