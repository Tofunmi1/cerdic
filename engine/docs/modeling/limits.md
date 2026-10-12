# Numeric limits and budget profile (v1, initial)

These are the caps enforced from G1 on. They are starting values chosen to be safe, not tuned: G2
measures them and revises this file with the evidence. A limit that is hit produces a `budget.*`
diagnostic naming the limit and the observed value; the document stays unchanged.

## Hard limits (checked before work begins)
| Limit | Value |
|---|---|
| Source text | 256 KiB |
| Document JSON | 2 MiB |
| Nodes per document (authored) | 5,000 |
| Expanded nodes after groups | 50,000 |
| Group nesting depth | 8 |
| Inputs on a variadic port | 64 |
| List parameter length | 4,096 values |
| String parameter | 256 bytes |
| Commands per batch / batch size | 200 / 256 KiB |
| History | 200 transactions or 16 MiB, whichever first |
| Profile loops / points per loop | 64 / 16,384 |
| Curve points / loft sections | 65,536 / 256 |
| Revolve segments / extrude divisions | 512 / 256 |
| Refine factor | 16, and the predicted triangle count must pass the geometry limit |
| Field grid cells | 8,000,000 (32 MiB of f32) |
| Field tape instructions | 4,096 |
| Triangles per result | 2,000,000 final, 200,000 preview |
| Subdivision output faces | 1,000,000 |
| Predicted realization | 4,000,000 triangles and 256 MiB |
| Kernel extent (exists) | `MAX_EXTENT` 1e6 m; 2^28 elements |

## Runtime caps
| Cap | Value |
|---|---|
| Pending worker queue | 8 jobs; a newer job supersedes older independent ones |
| Preview job | 2 s soft, terminated at 10 s |
| Final job | 30 s soft, terminated at 60 s |
| Result cache | 128 MiB, least recently used eviction |
| GPU upload per frame | 8 MiB and 2 ms of main-thread time |
| Worker restarts | 3 in 60 s, then the editor reports a persistent failure |
| Editor graph | 5,000 nodes loaded; interaction goal measured at 200 and 500 |

## Performance goals (targets until measured, on the baseline machine)
| Goal | Value |
|---|---|
| Warm preview p95, fixtures F1 and F2 | under 150 ms |
| Editor pan, zoom, drag, F3 (200 nodes) | display refresh, no frame over 33 ms |
| Moving a node | zero evaluations (instrumented) |
| Regeneration soak | 100 regenerations, bounded memory after warm-up |
| Long soak | 30 minutes of alternating edit, cancel and reload, editor open |
| Release size | recorded against 3.24 MB brotli (stripped) for every gate |

## Fixture manifest (names used by tests and reports)
| ID | Fixture | Size and workload |
|---|---|---|
| F1 | Vase (see `commands.md`) | 8 nodes, 64-segment revolve |
| F2 | Wheel assembly | 4 instances of one group, about 60 nodes; G4 |
| F3 | Synthetic 200-node graph | mixed chains and fans of cheap booleans and transforms |
| F4 | Synthetic 500-node graph | same generator, larger |
| F5 | Creature field | G8: 128 cubed grid, 6-primitive blend |
| F6 | Facade | G4 and G6: 40 instances, profile details |

Each performance report states: hardware, browser and version, build mode, warm or cold, fixture,
quality (preview or final), nodes, output triangles, cache hits and wall time (p50, p95, max).
