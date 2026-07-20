# Meta-approach 5k compare — reproducibility

- Created (UTC): `2026-07-20T04:21:47.015941+00:00`
- Git SHA: `c54a662ac95d3d861591fa071f6ecb406e0907e3`
- Seed: `1902771841`
- Iters: `5000` per approach
- Approaches: random, cmaes, reinforce, aging_evo, tpe, hybrid_lstm
  (manuscript: Random NAS, Cont. CMA-ES, Arch REINFORCE, Aging evolution, TPE Bayes NAS, Ours (hybrid GA–PPO))
- Vocab: LSTM + xLSTM in `BLOCKS`
- Reward modes (hybrid/REINFORCE/PBT): abs_r, vs_dualcosine, vs_nobake, neglog_gap
- Device: `NVIDIA GeForce RTX 3090` / torch `2.6.0+cu124`

Relaunch clean publishable run:

```bash
python scripts/launch_meta_approach_compare.py --iters 5000 --force --fresh
```

Poll:

```bash
python scripts/meta_approach_status.py
```

Live dashboard (Chart.js, polls STATUS + history every ~3s; also tails history → TensorBoard events):

```bash
python scripts/meta_approach_dashboard.py --open
# http://127.0.0.1:8765/
```

TensorBoard (after dashboard has synced, or once the bench writes events):

```bash
tensorboard --logdir brand/artifacts/meta_approach_compare/tb --port 6006
# http://127.0.0.1:6006/
```
