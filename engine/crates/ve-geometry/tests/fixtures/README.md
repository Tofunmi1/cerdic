# G6 real sliver meshes

These fixtures are our own procedural outputs from the lamp shade and goblet evaluator programs.
They preserve the triangle positions and indices emitted by the Manifold-backed evaluator, including
the f32-resolution sliver faces that motivated report-only validation.

Regenerate them from the saved big-test programs with:

```sh
cargo run -p ve-modeling --example export_g6_sliver_fixtures
```

The exporter reads `target/reports/big-test/programs/lamp.txt` and `goblet.txt` and writes the
versioned `VE_POLY_FIXTURE 1` text format here. It has no external model or asset source.
