<div align="center">

# CadFuture

**AI-native, LUT-first CAD — compute is the last resort.**

*A lightweight CAD, multi-physics, and manufacturing toolchain that lets physical-AI and embodied systems model the physics of their perceived world — and run the same model from a sub-5 mW neuromorphic chip to a workstation.*

[![Rust](https://img.shields.io/badge/Rust-edition_2024-cfaa5b?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-46e0c0?style=flat-square)](LICENSE)
[![Charlot Lab](https://img.shields.io/badge/Charlot_Lab-Physical_AI_%40_BMI-cfaa5b?style=flat-square)](https://labs.physicalai-bmi.org/charlot)

</div>

---

## Why

A world model is only actionable when a system can model the **physics** of it — will this hold, fit, overheat, deflect? Real CAD and simulation are too heavy to run where the body is. CadFuture's bet is that the physics an embodied agent needs is mostly **retrievable, not computable.**

## The one rule

> **LUT → Formula → Solver → LLM.** If the answer exists in a table, never compute it. If a closed-form formula exists, never iterate. If a sparse solve works, never invoke a model. GPU render only when there's a screen.

Most of engineering is lookup: standard components ≈ 100% precomputed, parametric features ≈ 95%, manufacturing constraints ≈ 100%. Every query walks the cascade, so the *typical* query costs picojoules, not joules.

## How it's built

- **One graph.** Parts, features, materials, processes, constraints, assemblies, tolerances, sensor readings — every entity is a node, every relationship an edge. The graph *is* the model; file formats are emit targets, not storage.
- **16 dimensions per node** — geometry (X/Y/Z), time, static/dynamic/thermal/electromagnetic fields, kinematic state, material identity, manufacturing process, tolerance, version, live-twin state, **agent interaction** (trajectory, grip, force, perception), and operational environment.
- **One model, every tier.** The same engineering truth runs from neuromorphic endpoints (constraints as spike thresholds, `no_std`) through MCUs and edge SBCs to mobile and desktop. The *execution strategy* adapts to the hardware; the engineering doesn't.

## Workspace

A Rust workspace (`edition = 2024`). The crates are organized by concern:

| Family | What it does |
|--------|--------------|
| `core/` | units, the **LUT engine**, constraints, the graph, the query cascade, tolerance, BOM, costing, standards, knowledge |
| `geometry/` | B-rep kernel, parametric features, sketches, tessellation |
| `simulation/` | analytical, FEA, CFD, EM, DFM, topology, SNN, digital-twin |
| `reasoning/` | inference, search, agent, surrogate models, generative, scan-to-CAD, intent |
| `emit/` | STEP, STL, DXF, 3MF, glTF, IGES, OBJ, Gerber, KiCad, G-code & more |
| `mfg/` | toolpaths, slicer, laser, CNC |
| `connect/` | machine connectivity (OctoPrint, Moonraker, Bambu, Duet, Haas, MTConnect, serial, …) |
| `render/` | wgpu viewport, instancing, LOD, overlays |
| `platform/` | native, web (WASM), and server entry points |

## Build

```bash
cargo build --workspace            # native
cargo build -p cad-platform-web --target wasm32-unknown-unknown   # web (WASM + WebGPU)
```

Core crates are `no_std` for embedded/neuromorphic tiers; higher tiers use `std`.

## Status

CadFuture is an **active research effort of [the Charlot Lab](https://labs.physicalai-bmi.org/charlot)** at the Institute for Physical AI, Bailey Military Institute — the *research effort* behind the lab's **Computable World Model** topic. It is under active development; interfaces will change.

**Used by** [**physics-mmast-sim**](https://github.com/dcharlot-physicalai-bmi/physics-mmast-sim) (the MMAST energy + signature simulator), which builds its vehicle geometry, tessellation, and thermal FEA on CadFuture's `physical-brep` / `physical-tessellation` / `physical-fea`.

## License

MIT © David Jean Charlot — see [LICENSE](LICENSE).

---

<div align="center">
<sub>The Charlot Lab · Institute for Physical AI · Bailey Military Institute</sub>
</div>
