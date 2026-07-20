#!/usr/bin/env python3
"""Live surveillance dashboard for meta-approach 5k compare (TensorBoard-like).

Serves a local HTML dashboard that auto-refreshes STATUS + champion-R curves
from brand/artifacts/meta_approach_compare/.

  .venv_gpu\\Scripts\\python.exe scripts\\meta_approach_dashboard.py
  # open http://127.0.0.1:8765/
"""
from __future__ import annotations

import argparse
import json
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / "brand" / "artifacts" / "meta_approach_compare"
APPROACHES = ("random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm")

HTML = r"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>DenoiseOpt meta-compare live</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js"></script>
<style>
  :root {
    --bg: #0f1419;
    --panel: #1a222c;
    --text: #e7ecf1;
    --muted: #8b9aab;
    --accent: #3d9cfd;
    --ok: #3dd68c;
    --warn: #f5a524;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0; font-family: "Segoe UI", system-ui, sans-serif;
    background: var(--bg); color: var(--text);
  }
  header {
    padding: 14px 20px; border-bottom: 1px solid #2a3542;
    display: flex; flex-wrap: wrap; gap: 12px 24px; align-items: baseline;
  }
  header h1 { font-size: 1.05rem; margin: 0; font-weight: 600; letter-spacing: 0.02em; }
  header .meta { color: var(--muted); font-size: 0.85rem; }
  header .live { color: var(--ok); font-weight: 600; }
  main { padding: 16px 20px 32px; display: grid; gap: 16px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 10px; }
  .card {
    background: var(--panel); border: 1px solid #2a3542; border-radius: 8px;
    padding: 12px 14px;
  }
  .card .label { color: var(--muted); font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; }
  .card .value { font-size: 1.25rem; margin-top: 4px; font-variant-numeric: tabular-nums; }
  .card .sub { color: var(--muted); font-size: 0.8rem; margin-top: 2px; }
  .bar-wrap { height: 6px; background: #2a3542; border-radius: 3px; margin-top: 8px; overflow: hidden; }
  .bar { height: 100%; background: linear-gradient(90deg, var(--accent), var(--ok)); width: 0%; }
  .chart-wrap { background: var(--panel); border: 1px solid #2a3542; border-radius: 8px; padding: 12px 14px; }
  .chart-wrap h2 { margin: 0 0 10px; font-size: 0.95rem; font-weight: 600; }
  canvas { max-height: 360px; }
  table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
  th, td { text-align: left; padding: 8px 10px; border-bottom: 1px solid #2a3542; }
  th { color: var(--muted); font-weight: 500; font-size: 0.75rem; text-transform: uppercase; }
  td.num { font-variant-numeric: tabular-nums; }
  .active { color: var(--accent); font-weight: 600; }
  .done { color: var(--ok); }
  footer { color: var(--muted); font-size: 0.75rem; padding: 0 20px 20px; }
</style>
</head>
<body>
<header>
  <h1>DenoiseOpt · meta-approach compare</h1>
  <span class="meta" id="phase">loading…</span>
  <span class="meta live" id="live">LIVE</span>
  <span class="meta" id="updated"></span>
</header>
<main>
  <div class="grid" id="kpis"></div>
  <div class="chart-wrap">
    <h2>Champion residual R vs outer iteration</h2>
    <canvas id="champChart"></canvas>
  </div>
  <div class="chart-wrap">
    <h2>Per-approach progress</h2>
    <table>
      <thead>
        <tr>
          <th>Approach</th><th>Done</th><th>%</th><th>Champ R</th>
          <th>LSTM</th><th>xLSTM</th><th>Wall-h</th><th>State</th>
        </tr>
      </thead>
      <tbody id="rows"></tbody>
    </table>
  </div>
</main>
<footer>
  Auto-refresh ~3s · data from STATUS.json + history.jsonl ·
  <span id="outDir"></span>
</footer>
<script>
const COLORS = {
  random: '#56B4E9',
  cmaes: '#E69F00',
  reinforce: '#009E73',
  aging_evo: '#CC79A7',
  tpe: '#0072B2',
  hybrid_lstm: '#D55E00',
};
let chart;

async function fetchJSON(url) {
  const r = await fetch(url, { cache: 'no-store' });
  if (!r.ok) throw new Error(url + ' ' + r.status);
  return r.json();
}

function fmt(x, d=5) {
  if (x === null || x === undefined || Number.isNaN(x)) return '—';
  return Number(x).toFixed(d);
}

function renderStatus(st) {
  document.getElementById('phase').textContent =
    `phase=${st.phase || '?'} · ${st.n_complete || 0}/${st.n_total || 0} complete · current=${st.current_approach || '—'}@${st.current_iter ?? '—'}`;
  document.getElementById('updated').textContent = st.updated_at || '';
  document.getElementById('outDir').textContent = st.out_dir || '';
  const rows = st.rows || [];
  const active = rows.find(r => r.approach === st.current_approach) || rows.find(r => r.iters_done > 0) || {};
  const kpis = [
    { label: 'Current', value: st.current_approach || '—', sub: `iter ${st.current_iter ?? 0}` },
    { label: 'Champ R', value: fmt(active.champ_r), sub: active.approach || '' },
    { label: 'Progress', value: `${st.n_complete || 0}/${st.n_total || 0}`, sub: 'approaches done' },
    { label: 'Target', value: String(st.target_iters || 5000), sub: 'iters / approach' },
  ];
  document.getElementById('kpis').innerHTML = kpis.map(k => `
    <div class="card"><div class="label">${k.label}</div>
    <div class="value">${k.value}</div><div class="sub">${k.sub}</div></div>`).join('');

  document.getElementById('rows').innerHTML = rows.map(r => {
    const pct = r.pct || 0;
    let state = r.complete ? 'DONE' : (r.iters_done ? 'RUN' : 'PEND');
    if (st.current_approach === r.approach && !r.complete) state = 'ACTIVE';
    const cls = state === 'ACTIVE' ? 'active' : (state === 'DONE' ? 'done' : '');
    return `<tr>
      <td class="${cls}">${r.approach}</td>
      <td class="num">${r.iters_done || 0}/${r.target_iters || 0}</td>
      <td class="num">${pct.toFixed(1)}%
        <div class="bar-wrap"><div class="bar" style="width:${Math.min(100,pct)}%"></div></div>
      </td>
      <td class="num">${fmt(r.champ_r)}</td>
      <td>${r.lstm_in_champ ? 'Y' : 'n'}</td>
      <td>${r.xlstm_in_champ ? 'Y' : 'n'}</td>
      <td class="num">${((r.wall_s||0)/3600).toFixed(2)}</td>
      <td class="${cls}">${state}</td>
    </tr>`;
  }).join('');
}

function renderCurves(curves) {
  const datasets = Object.entries(curves.series || {}).map(([name, pts]) => ({
    label: name,
    data: pts.map(p => ({ x: p.iter, y: p.champ })),
    borderColor: COLORS[name] || '#ccc',
    backgroundColor: COLORS[name] || '#ccc',
    pointRadius: 0,
    borderWidth: 2,
    tension: 0.15,
  }));
  if (curves.baseline_dual_cosine != null) {
    datasets.push({
      label: 'DualCosine',
      data: [{ x: 0, y: curves.baseline_dual_cosine }, { x: curves.target_iters || 5000, y: curves.baseline_dual_cosine }],
      borderColor: '#888',
      borderDash: [6, 4],
      pointRadius: 0,
      borderWidth: 1.5,
    });
  }
  if (curves.baseline_nobake != null) {
    datasets.push({
      label: 'no-bake',
      data: [{ x: 0, y: curves.baseline_nobake }, { x: curves.target_iters || 5000, y: curves.baseline_nobake }],
      borderColor: '#555',
      borderDash: [2, 3],
      pointRadius: 0,
      borderWidth: 1.2,
    });
  }
  const ctx = document.getElementById('champChart');
  if (!chart) {
    chart = new Chart(ctx, {
      type: 'line',
      data: { datasets },
      options: {
        animation: false,
        parsing: false,
        normalized: true,
        scales: {
          x: {
            type: 'linear', min: 0, max: curves.target_iters || 5000,
            title: { display: true, text: 'Outer iteration', color: '#8b9aab' },
            ticks: { color: '#8b9aab' }, grid: { color: '#2a3542' },
          },
          y: {
            min: 0.7, max: 1.0,
            title: { display: true, text: 'Champion R', color: '#8b9aab' },
            ticks: { color: '#8b9aab' }, grid: { color: '#2a3542' },
          },
        },
        plugins: {
          legend: { labels: { color: '#e7ecf1', boxWidth: 12 } },
        },
      },
    });
  } else {
    chart.data.datasets = datasets;
    chart.options.scales.x.max = curves.target_iters || 5000;
    chart.update('none');
  }
}

async function tick() {
  try {
    const [st, curves] = await Promise.all([
      fetchJSON('/api/status'),
      fetchJSON('/api/curves'),
    ]);
    renderStatus(st);
    renderCurves(curves);
    document.getElementById('live').textContent = 'LIVE';
    document.getElementById('live').style.color = '#3dd68c';
  } catch (e) {
    document.getElementById('live').textContent = 'STALE';
    document.getElementById('live').style.color = '#f5a524';
    console.error(e);
  }
}
tick();
setInterval(tick, 3000);
</script>
</body>
</html>
"""


def load_status(out_dir: Path) -> dict:
    path = out_dir / "STATUS.json"
    if not path.is_file():
        return {
            "schema": "denoiseopt.meta_approach_status.v1",
            "phase": "waiting",
            "rows": [{"approach": a, "iters_done": 0, "target_iters": 5000, "pct": 0.0} for a in APPROACHES],
            "n_complete": 0,
            "n_total": len(APPROACHES),
            "out_dir": str(out_dir),
        }
    return json.loads(path.read_text(encoding="utf-8"))


def load_curves(out_dir: Path, stride: int = 1) -> dict:
    series: dict[str, list[dict]] = {}
    baseline_dc = None
    baseline_nb = None
    target = 5000
    for name in APPROACHES:
        hist = out_dir / name / "history.jsonl"
        pts: list[dict] = []
        if hist.is_file():
            with hist.open(encoding="utf-8") as f:
                for i, line in enumerate(f):
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        row = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if baseline_dc is None and row.get("baseline_dual_cosine") is not None:
                        baseline_dc = float(row["baseline_dual_cosine"])
                    if baseline_nb is None and row.get("baseline_nobake") is not None:
                        baseline_nb = float(row["baseline_nobake"])
                    pts.append(
                        {
                            "iter": int(row.get("iter", i + 1)),
                            "champ": float(row.get("champ_raw", row.get("champ", row.get("residual", 0.0)))),
                            "trial": float(row.get("residual", row.get("r_raw", 0.0)))
                            if row.get("residual") is not None or row.get("r_raw") is not None
                            else None,
                            "wall_s": float(row["wall_s"]) if row.get("wall_s") is not None else None,
                        }
                    )
        if stride > 1 and len(pts) > 400:
            pts = pts[:: max(1, len(pts) // 400)] + ([pts[-1]] if pts else [])
        if pts:
            series[name] = pts
        ckpt = out_dir / name / "checkpoint.json"
        if ckpt.is_file():
            try:
                c = json.loads(ckpt.read_text(encoding="utf-8"))
                if c.get("baseline_dual_cosine") is not None:
                    baseline_dc = float(c["baseline_dual_cosine"])
            except Exception:
                pass
    st = load_status(out_dir)
    target = int(st.get("target_iters") or target)
    return {
        "series": series,
        "baseline_dual_cosine": baseline_dc,
        "baseline_nobake": baseline_nb,
        "target_iters": target,
        "updated_at": time.time(),
    }


def tb_sync_loop(out_dir: Path, stop: threading.Event, interval_s: float = 5.0) -> None:
    """Tail history.jsonl → TensorBoard events (works without restarting the bench)."""
    try:
        from torch.utils.tensorboard import SummaryWriter
    except Exception as e:
        print(f"TB sync disabled ({e})", flush=True)
        return
    tb_dir = out_dir / "tb"
    tb_dir.mkdir(parents=True, exist_ok=True)
    writers: dict[str, Any] = {}
    offsets: dict[str, int] = {a: 0 for a in APPROACHES}
    print(f"TB sync -> {tb_dir}", flush=True)
    while not stop.is_set():
        for name in APPROACHES:
            hist = out_dir / name / "history.jsonl"
            if not hist.is_file():
                continue
            try:
                data = hist.read_bytes()
            except OSError:
                continue
            if len(data) <= offsets[name]:
                continue
            chunk = data[offsets[name] :]
            offsets[name] = len(data)
            if name not in writers:
                writers[name] = SummaryWriter(log_dir=str(tb_dir / name))
            w = writers[name]
            for line in chunk.splitlines():
                line = line.strip()
                if not line:
                    continue
                try:
                    row = json.loads(line)
                except json.JSONDecodeError:
                    continue
                it = int(row.get("iter", 0))
                if it <= 0:
                    continue
                champ = float(row.get("champ_raw", row.get("champ", row.get("residual", 0.0))))
                trial = row.get("residual", row.get("r_raw"))
                wall = row.get("wall_s")
                w.add_scalar("champ_R", champ, it)
                if trial is not None:
                    w.add_scalar("trial_R", float(trial), it)
                if wall is not None:
                    w.add_scalar("wall_h", float(wall) / 3600.0, it)
            try:
                w.flush()
            except Exception:
                pass
        stop.wait(interval_s)
    for w in writers.values():
        try:
            w.close()
        except Exception:
            pass


def make_handler(out_dir: Path):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, fmt: str, *args) -> None:  # quieter
            if "/api/" not in (args[0] if args else ""):
                return

        def _send(self, code: int, body: bytes, ctype: str) -> None:
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self) -> None:  # noqa: N802
            path = urlparse(self.path).path
            if path in ("/", "/index.html"):
                self._send(200, HTML.encode("utf-8"), "text/html; charset=utf-8")
                return
            if path == "/api/status":
                body = json.dumps(load_status(out_dir)).encode("utf-8")
                self._send(200, body, "application/json")
                return
            if path == "/api/curves":
                qs = parse_qs(urlparse(self.path).query)
                stride = int(qs.get("stride", ["1"])[0])
                body = json.dumps(load_curves(out_dir, stride=stride)).encode("utf-8")
                self._send(200, body, "application/json")
                return
            if path == "/api/manifest":
                man = out_dir / "REPRO_MANIFEST.json"
                if man.is_file():
                    self._send(200, man.read_bytes(), "application/json")
                else:
                    self._send(404, b'{"error":"no manifest"}', "application/json")
                return
            self._send(404, b"not found", "text/plain")

    return Handler


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out-dir", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--open", action="store_true", help="Open browser")
    ap.add_argument("--no-tb", action="store_true", help="Disable TensorBoard event sync")
    args = ap.parse_args()
    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    stop = threading.Event()
    if not args.no_tb:
        threading.Thread(target=tb_sync_loop, args=(out_dir, stop), daemon=True).start()

    handler = make_handler(out_dir)
    server = ThreadingHTTPServer((args.host, args.port), handler)
    url = f"http://{args.host}:{args.port}/"
    print(f"LIVE dashboard: {url}", flush=True)
    print(f"Watching: {out_dir}", flush=True)
    if args.open:
        threading.Timer(0.6, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        stop.set()
        print("\nstopped", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
