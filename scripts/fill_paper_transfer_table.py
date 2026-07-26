# -*- coding: utf-8 -*-
"""Fill results_transfer.tex table from signal_heal_transfer/results_table.json."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RES = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "results_table.json"
PAPER = (
    ROOT.parent
    / "denoise-opt-meta"
    / "paper"
    / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9"
    / "subsections"
    / "results_transfer.tex"
)

COLS = [
    ("cwru_bearings", "CWRU"),
    ("mfpt_bearings", "MFPT"),
    ("mitbih_ecg", "MIT-BIH"),
    ("ptbxl_ecg", "PTB-XL"),
    ("synth_cnc_g01", "synth CNC"),
    ("synth_pmu_cycle", "synth PMU"),
]
ROWS = [
    ("ours_hybrid_lstm", r"Ours (\texttt{hybrid\_lstm})"),
    ("no_bake", "no-bake"),
    ("endpoint_pin_mean", "endpoint-pin"),
    ("seam_fir3", r"\texttt{seam\_fir3}"),
    ("dual_cosine", "DualCosine"),
]


def fmt(v) -> str:
    if v is None:
        return "---"
    try:
        x = float(v)
    except Exception:
        return "---"
    if x != x:
        return "---"
    return f"{x:.4f}"


def main() -> int:
    blob = json.loads(RES.read_text(encoding="utf-8"))
    table = blob.get("table") or {}
    lines = [
        r"% Auto-filled from brand/artifacts/signal_heal_transfer/results_table.json",
        r"\subsection{Sci/eng wrap-heal transfer results}",
        r"\label{sec:transfer-main}",
        "",
        r"\paragraph{Classical-board disclaimer.}",
        r"Table~\ref{tab:transfer-main} reports holdout-refit prolonged $R$ on classical / domain-proxy boards only.",
        r"Deep SOTA rows are listed as \emph{not executed} (Table~\ref{tab:transfer-sota-status}).",
        r"Synthetic CNC/PMU rows are protocol pilots when real KIT / DataPort downloads are blocked.",
        "",
        r"\begin{table*}[t]",
        r"  \centering",
        r"  \caption{Sci/eng wrap-heal transfer: holdout-refit prolonged $R$ (higher better) on classical boards.",
        r"    Ours = \texttt{hybrid\_lstm}. Prior CWRU/MIT-BIH: 250 outer iters; MFPT/PTB-XL/synth: 150 outer iters this session.",
        r"    Classical board only.}",
        r"  \label{tab:transfer-main}",
        r"  \setlength{\tabcolsep}{2.8pt}",
        r"  \begin{tabular}{@{}lrrrrrr@{}}",
        r"    \toprule",
        r"    Method & CWRU & MFPT & MIT-BIH & PTB-XL & synth CNC & synth PMU \\",
        r"    \midrule",
    ]
    for key, label in ROWS:
        cells = []
        for ds, _ in COLS:
            row = table.get(ds) or {}
            cells.append(fmt(row.get(key)))
        lines.append(f"    {label} & " + " & ".join(cells) + r" \\")
    lines += [
        r"    \bottomrule",
        r"  \end{tabular}",
        r"\end{table*}",
        "",
        r"\begin{figure}[t]",
        r"  \centering",
        r"  \includegraphics[width=\columnwidth]{figures/fig_signal_heal_transfer.png}",
        r"  \caption{Transfer classical-board summary (holdout-refit $R$).",
        r"    Not a deep-SOTA claim; see Table~\ref{tab:transfer-sota-status}.}",
        r"  \label{fig:signal-heal-transfer-main}",
        r"\end{figure}",
        "",
        r"\begin{table}[t]",
        r"  \centering",
        r"  \caption{Deep / real-data transfer status under this residual protocol.",
        r"    No invented scores.}",
        r"  \label{tab:transfer-sota-status}",
        r"  \setlength{\tabcolsep}{3pt}",
        r"  \begin{tabular}{@{}lp{0.62\columnwidth}@{}}",
        r"    \toprule",
        r"    Item & Status \\",
        r"    \midrule",
        r"    Cycle-GAN (ECG) & \emph{not executed} (no weights under $R$) \\",
        r"    BeatDiff & \emph{not executed} \\",
        r"    Paderborn KAt deep & \emph{not executed} ($K001$ extracted; deep unwired) \\",
        r"    Full PTB-XL 500\,Hz & deferred (ran \texttt{records100} subset) \\",
        r"    Real KIT CNC / IEEE PMU & deferred (login walls; synth pilots ran) \\",
        r"    Formal MOS/MUSHRA & deferred (Section~\ref{sec:listening-protocol}) \\",
        r"    \bottomrule",
        r"  \end{tabular}",
        r"\end{table}",
        "",
        r"\paragraph{Reading.}",
        r"On CWRU, DualCosine/fades hurt vs no-bake; Ours only narrowly beats no-bake (weak transfer on that wrap).",
        r"On MIT-BIH, DualCosine collapses morphology while Ours leads the classical board.",
        r"MFPT extends the bearings classical board with a fixed-shaft public set.",
        r"PTB-XL \texttt{records100} is a low-res ECG subset pilot, not the full 500\,Hz corpus.",
        r"Synthetic CNC/PMU show the same residual protocol on G01-corner and AC-cycle wrap proxies.",
        r"Primary claim remains cycle-local \emph{wavetable} seam restoration.",
        "",
        r"\subsection{Listening / spectrogram protocol (main Results)}",
        r"\label{sec:listening-protocol}",
        "",
        r"We place audible and spectrogram evidence in the main Results path (Sections~\ref{sec:vibrato-eval}--\ref{sec:hear-samples}), not only an appendix note.",
        r"Downloadable WAVs (no-bake / DualCosine / Ours, A4, $3$\,s) and vibrato spectrograms support informal A/B inspection.",
        r"\textbf{Formal MOS / MUSHRA scores were not collected in this work.}",
        r"A future formal listening study would freeze the same WAV pack, recruit listeners under a MUSHRA-style wrap-click impairment scale, and report mean scores with confidence intervals.",
        r"Until that study runs, perceptual claims remain qualitative and secondary to prolonged $R$.",
        "",
    ]
    PAPER.write_text("\n".join(lines), encoding="utf-8")
    print("wrote", PAPER)
    print("datasets", sorted(table.keys()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
