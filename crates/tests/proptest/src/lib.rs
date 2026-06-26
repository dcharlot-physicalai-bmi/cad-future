//! SQLite-grade property testing for the OpenIE CAD kernel.
//!
//! Every geometric invariant is tested over thousands of random inputs.
//! When a failure is found, proptest automatically shrinks it to the
//! minimal reproducing case.
//!
//! # Invariant Categories
//!
//! 1. **B-Rep topology** — Euler formula, manifold closure, positive area
//! 2. **Boolean operations** — volume bounds, manifold preservation
//! 3. **Tessellation** — bounding box containment, no degenerate triangles
//! 4. **Sketch solver** — convergence, DOF correctness, fixed-point stability
//! 5. **FEA** — zero-load-zero-displacement, finite stress, energy conservation
//! 6. **CFL** — parse safety, execute safety, lex roundtrip

#[cfg(test)]
mod brep_invariants;
#[cfg(test)]
mod boolean_invariants;
#[cfg(test)]
mod tessellation_invariants;
#[cfg(test)]
mod sketch_invariants;
#[cfg(test)]
mod fea_invariants;
#[cfg(test)]
mod cfl_invariants;
