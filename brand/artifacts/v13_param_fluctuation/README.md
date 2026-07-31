# v13 champion parameter fluctuation

Outer-loop champions are **not** a fixed size. Graph search jumps between compact
and heavy cells; replication across seeds can land on different param counts.

- **v10 locked hybrid (old primary Ours):** 121451 params, R_blend=0.969511866569519
## seed 1902771841
- **hybrid_lstm** iters=5000: champ-update params min/med/max = 15481/198187.0/373077; live champ = 25756 params, R=0.9766173660755157, beats_n2n=True
- **random** iters=5000: champ-update params min/med/max = 5450/153930.0/292238; live champ = 197053 params, R=0.9403516352176666, beats_n2n=False
- **cmaes** iters=5000: champ-update params min/med/max = 15920/59272.0/219294; live champ = 59272 params, R=0.95550137758255, beats_n2n=False
- **tpe** iters=5000: champ-update params min/med/max = 25356/75888.0/169088; live champ = 120754 params, R=0.9461058676242828, beats_n2n=False
- **aging_evo** iters=5000: champ-update params min/med/max = 7015/98308.0/344708; live champ = 160378 params, R=0.9632943272590637, beats_n2n=False
- **reinforce** iters=5000: champ-update params min/med/max = 5928/119504.0/210368; live champ = 210368 params, R=0.953599363565445, beats_n2n=False

## seed 2026072701
- **hybrid_lstm** iters=5000: champ-update params min/med/max = 8746/26782.0/249166; live champ = 25271 params, R=0.973026692867279, beats_n2n=False
- **random** iters=5000: champ-update params min/med/max = 35464/115292.0/236712; live champ = 122148 params, R=0.9352470636367798, beats_n2n=False
- **cmaes** iters=5000: champ-update params min/med/max = 10250/64865.0/215522; live champ = 67212 params, R=0.9556275010108948, beats_n2n=False
- **tpe** iters=5000: champ-update params min/med/max = 28208/129038.0/213032; live champ = 63616 params, R=0.9463729560375214, beats_n2n=False
- **aging_evo** iters=5000: champ-update params min/med/max = 43854/120791.0/269641; live champ = 269641 params, R=0.9586635828018188, beats_n2n=False
- **reinforce** iters=5000: champ-update params min/med/max = 86648/123588.0/229053; live champ = 86648 params, R=0.9732869863510132, beats_n2n=False

## seed 2026072702
- **hybrid_lstm** iters=5000: champ-update params min/med/max = 21419/41120.0/270557; live champ = 21419 params, R=0.9748305976390839, beats_n2n=False
- **random** iters=5000: champ-update params min/med/max = 25512/86394.0/183154; live champ = 52710 params, R=0.9345309734344482, beats_n2n=False
- **cmaes** iters=5000: champ-update params min/med/max = 27256/109515.0/206444; live champ = 39524 params, R=0.9582505524158478, beats_n2n=False
- **tpe** iters=5000: champ-update params min/med/max = 13276/120367.0/237482; live champ = 104924 params, R=0.9429396390914917, beats_n2n=False
- **aging_evo** iters=5000: champ-update params min/med/max = 6824/105396.0/287050; live champ = 30776 params, R=0.9684269726276398, beats_n2n=False
- **reinforce** iters=5000: champ-update params min/med/max = 5397/204770.0/319139; live champ = 319139 params, R=0.9527857005596161, beats_n2n=False

