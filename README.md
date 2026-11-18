# AreaClicker

A Windows desktop tool in Rust (egui) for scripting repetitive mouse input over user-selected screen areas, for long-running UI soak tests. Click areas compose into sequences that are compiled and validated before execution: recursive-sequence detection, empty-area checks and delay normalisation. Built in 2025 for an extracurricular project that was later dropped.

**Status: archived, not maintained.**

## Features

- Click areas and sequences saved to a TOML library
- Sequences can contain other sequences; cycles are rejected before a run starts
- Each run is compiled into a flat, validated plan first
- Straight-line cursor movement at constant speed, then a click
- Pause/resume (`F3`) and stop at any time; every wait is interruptible

## Build

Requires Windows and a recent stable Rust toolchain.

```
cargo build --release
cargo test
```

The binary is `target/release/area_clicker.exe`. Presets are stored in `%APPDATA%/AreaClicker/AreaClicker/ui_presets.toml`.

## Layout

- `src/main.rs`: UI and run loop
- `src/presets.rs`: preset models and storage
- `src/domain/validation.rs`: run-plan compilation and validation
- `src/cursor.rs`: cursor movement and clicking
