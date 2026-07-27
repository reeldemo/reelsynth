# v13 champion parameter fluctuation

Outer-loop champions are **not** a fixed size. Graph search jumps between compact
and heavy cells; replication across seeds can land on different param counts.

- **v10 locked hybrid (old primary Ours):** 121451 params, R_blend=0.969511866569519
## seed 1902771841
- **hybrid_lstm** iters=2875: champ-update params min/med/max = 15481/198187.0/373077; live champ = 25756 params, R=0.9766173660755157, beats_n2n=True

## seed 2026072701
- **hybrid_lstm** iters=None: champ-update params min/med/max = 156982/166132.0/249166; live champ = 136484 params, R=0.9156511425971985, beats_n2n=False

