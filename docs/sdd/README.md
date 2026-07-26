# SDD layout (ReelSynth)

```
docs/sdd/
  CONSTITUTION.md
  specs/<feature>/
    requirements.md   # what (user stories + AC)
    spec.md           # how (technical architecture) — NOT visual design
    tasks.md          # ordered implementation units
    design.md         # OPTIONAL — visual/UI/UX only
    analyze.md        # optional drift report
```

| File | Meaning |
|------|---------|
| `spec.md` | Technical plan / architecture (`sdd-plan`) |
| `design.md` | Visual / UI design only — do not put technical architecture here |
| `requirements.md` | User stories + acceptance criteria (`sdd-specify`) |
| `tasks.md` | Executable task table (`sdd-tasks`) |
