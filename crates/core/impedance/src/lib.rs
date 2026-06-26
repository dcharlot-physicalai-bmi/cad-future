//! `physical-impedance` — The periodic stack of computation as a measurement tool.
//!
//! Every operation has a natural thermodynamic cost (its **impedance**) and a
//! current implementation cost. The gap between the two is wasted energy.
//!
//! This crate makes that gap measurable. You register an operation with its
//! current coordinate (primitive id, abstraction level, concurrency topology),
//! and the analyzer reports:
//!
//! 1. The natural coordinate it should occupy
//! 2. The impedance gap (multiplier — how much more energy the current
//!    implementation costs than the floor)
//! 3. The concrete fix (which level/topology change closes the gap)
//!
//! # The framework
//!
//! Three orthogonal axes describe every computational operation:
//!
//! - **Primitive** — what the operation IS (matmul, scatter, comparison, ...)
//! - **Abstraction level (B)** — where it runs (B₀ physics → B₄ software)
//! - **Concurrency topology (D)** — how it parallelizes
//! - **Information topology (F)** — bijective (free) vs reductive (Landauer-bounded)
//!
//! The [`thermodynamic_floor`] of an operation is determined by its information
//! topology — bijective operations are L₀ (zero), reductive operations are L₂
//! (k·T·ln 2 per bit erased). The [`actual_cost`] depends on the abstraction
//! level it's currently running at — software at B₄ pays full overhead, hardware
//! at B₁ pays close to the floor.
//!
//! # Example
//!
//! ```
//! use physical_impedance::*;
//!
//! // FEA stiffness matrix assembly: currently a CPU loop in Rust
//! let op = Operation {
//!     name: "fea_stiffness_assembly".into(),
//!     primitive: Primitive::DenseMatmul,
//!     abstraction: AbstractionLevel::B4Software,
//!     topology: ConcurrencyTopology::D5Sequential,
//!     info_topology: InformationTopology::Reductive,
//! };
//!
//! let analysis = analyze(&op);
//! // analysis.gap_orders > 0 because B4 sequential dense matmul is paying
//! // far more than the L2 floor for what is structurally a sparse parallel op
//! assert!(analysis.gap_orders > 0.0);
//! ```

use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// The compute primitives — a subset of David Charlot's Periodic Stack.
// Each primitive has a fixed information-topology family, which determines its
// thermodynamic floor. Identifiers match the IDs referenced in the source notes.
// ---------------------------------------------------------------------------

/// Computational primitives, indexed by ID.
///
/// IDs match David Charlot's *Periodic Stack of Computation* — only the
/// primitives currently exercised by cad-future are encoded here. Add new
/// variants as new operations are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Primitive {
    // ---- ALU family ----
    /// #1 Integer arithmetic — bijective on the integer ring (modular).
    IntegerArith,
    /// #2 Bitwise logic — XOR, AND, OR, shift. Reversible. **L₀**.
    BitwiseLogic,
    /// #3 Float arithmetic — IEEE-754 add/mul. Reductive (rounding). L₂.
    FloatArith,
    /// #4 Fused multiply-add. L₂.
    FusedMulAdd,
    /// #5 Comparison / predicate. Reductive (n→1 bit). L₂max.
    ComparisonPredicate,
    /// #7 Quantize — discretize a continuous value. Reductive. L₂.
    Quantize,

    // ---- Linear algebra ----
    /// #18 Dense matrix multiply. Reductive (sums). L₂.
    DenseMatmul,
    /// #19 Sparse matrix multiply. Reductive but skips zeros.
    SparseMatmul,
    /// #20 General matmul (alias of #18 used by attention).
    Matmul,
    /// #21 Reduction (sum, max, mean across axis). Reductive. L₂max.
    Reduction,

    // ---- Spectral / transforms ----
    /// #12 Number-theoretic transform / orthogonal transform. **L₀** (bijective).
    NttTransform,
    /// #23 FFT. Bijective up to numerical precision. **L₀**.
    Fft,
    /// #24 Convolution (dense kernel over signal). Reductive. L₂.
    Convolution,
    /// #25 Wavelet / spectral decomposition. **L₀** (bijective).
    Wavelet,

    // ---- Nonlinear ----
    /// #33 Activation function (ReLU, GELU, sigmoid, tanh). NL. L₂.
    Activation,
    /// #34 Softmax / normalization. Reductive. L₂.
    Softmax,
    /// #35 Layer normalization. Reductive. L₂.
    LayerNorm,

    // ---- Probabilistic ----
    /// #37 Random number generation. Requires entropy source. L₁.
    Rng,
    /// #38 Monte-Carlo accept/reject. Stochastic. L₂.
    MonteCarlo,

    // ---- Memory / indexing ----
    /// #40 Scatter / gather — write/read indexed. **L₀** (bijective rearrangement).
    ScatterGather,
    /// #41 Embedding lookup — table read by index. **L₀**.
    EmbeddingLookup,
    /// #42 Finite-automaton step (tokenizer, regex, parser). L₂.
    FiniteAutomaton,
    /// #43 Relational join — match rows by key. L₂ (reductive but deterministic).
    RelationalJoin,
}

impl Primitive {
    /// Numeric ID matching the periodic stack.
    pub fn id(&self) -> u32 {
        match self {
            Self::IntegerArith => 1, Self::BitwiseLogic => 2, Self::FloatArith => 3,
            Self::FusedMulAdd => 4, Self::ComparisonPredicate => 5, Self::Quantize => 7,
            Self::NttTransform => 12, Self::DenseMatmul => 18, Self::SparseMatmul => 19,
            Self::Matmul => 20, Self::Reduction => 21, Self::Fft => 23,
            Self::Convolution => 24, Self::Wavelet => 25, Self::Activation => 33,
            Self::Softmax => 34, Self::LayerNorm => 35, Self::Rng => 37,
            Self::MonteCarlo => 38, Self::ScatterGather => 40, Self::EmbeddingLookup => 41,
            Self::FiniteAutomaton => 42, Self::RelationalJoin => 43,
        }
    }

    /// Information topology determines the thermodynamic floor.
    pub fn info_family(&self) -> InformationTopology {
        match self {
            // Bijective — no information erased — L₀
            Self::BitwiseLogic | Self::NttTransform | Self::Fft
            | Self::Wavelet | Self::ScatterGather | Self::EmbeddingLookup => {
                InformationTopology::Bijective
            }
            // Stochastic — needs entropy source — L₁
            Self::Rng => InformationTopology::Stochastic,
            // Everything else erases information — L₂
            _ => InformationTopology::Reductive,
        }
    }

    /// The natural concurrency topology for this primitive — the one its
    /// structure was designed to exploit.
    pub fn natural_topology(&self) -> ConcurrencyTopology {
        match self {
            Self::DenseMatmul | Self::Matmul | Self::Convolution | Self::LayerNorm
                => ConcurrencyTopology::D2Matrix,
            Self::SparseMatmul | Self::RelationalJoin
                => ConcurrencyTopology::D3Graph,
            Self::ScatterGather | Self::EmbeddingLookup | Self::Activation
            | Self::Softmax | Self::ComparisonPredicate | Self::Quantize
            | Self::FloatArith | Self::FusedMulAdd | Self::IntegerArith
            | Self::BitwiseLogic
                => ConcurrencyTopology::D4EmbarrassinglyParallel,
            Self::Fft | Self::NttTransform | Self::Wavelet | Self::Reduction
                => ConcurrencyTopology::D1Vector,
            Self::Rng | Self::MonteCarlo | Self::FiniteAutomaton
                => ConcurrencyTopology::D5Sequential,
        }
    }

    /// The natural abstraction level — where this primitive runs cheapest.
    /// Most primitives have hardware support; only a few are inherently software.
    pub fn natural_abstraction(&self) -> AbstractionLevel {
        match self {
            // These have dedicated silicon (TMU, ALU, FPU)
            Self::EmbeddingLookup | Self::ScatterGather | Self::BitwiseLogic
            | Self::IntegerArith | Self::ComparisonPredicate | Self::FloatArith
            | Self::FusedMulAdd
                => AbstractionLevel::B1Hardware,
            // Tensor cores, geometry shaders, sampler hardware
            Self::DenseMatmul | Self::Matmul | Self::Convolution | Self::Activation
            | Self::Softmax | Self::LayerNorm | Self::Fft | Self::NttTransform
            | Self::Reduction | Self::Wavelet | Self::Quantize
                => AbstractionLevel::B2Microarch,
            // Need ISA-level dispatch but no dedicated silicon
            Self::SparseMatmul | Self::RelationalJoin | Self::Rng
                => AbstractionLevel::B3Isa,
            // Stochastic search — inherently software-level
            Self::MonteCarlo | Self::FiniteAutomaton
                => AbstractionLevel::B4Software,
        }
    }
}

// ---------------------------------------------------------------------------
// Axes
// ---------------------------------------------------------------------------

/// Information topology — what happens to the information through the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InformationTopology {
    /// F₁ — bijective. No information erased. **Thermodynamic floor: L₀ (free).**
    Bijective,
    /// Stochastic. Requires an entropy source. L₁.
    Stochastic,
    /// F_N1 — reductive. N inputs → 1 output. Landauer cost per bit erased. L₂.
    Reductive,
}

impl InformationTopology {
    /// Thermodynamic floor in `LandauerLevel`.
    pub fn floor(&self) -> LandauerLevel {
        match self {
            Self::Bijective => LandauerLevel::L0Free,
            Self::Stochastic => LandauerLevel::L1Entropy,
            Self::Reductive => LandauerLevel::L2Landauer,
        }
    }
}

/// Thermodynamic cost level. L₀ is reversible (free in principle); L₂ pays
/// k·T·ln 2 per erased bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LandauerLevel {
    /// L₀ — bijective, reversible, zero floor. Lookup, FFT, scatter/gather.
    L0Free,
    /// L₁ — entropy required (RNG, sampling).
    L1Entropy,
    /// L₂ — Landauer-bounded (information erased). Comparisons, reductions, softmax.
    L2Landauer,
    /// L₂max — multiple-bit reduction (comparisons over wide vectors).
    L2Max,
}

impl LandauerLevel {
    /// Numeric cost order — used for impedance gap math.
    /// Returns the *minimum* energy cost order of magnitude relative to L₀.
    pub fn cost_order(&self) -> f64 {
        match self {
            Self::L0Free => 0.0,
            Self::L1Entropy => 1.0,
            Self::L2Landauer => 2.0,
            Self::L2Max => 2.5,
        }
    }
}

/// Where the operation is executed in the abstraction stack.
/// B₀ is the physical substrate; B₄ is interpreted software.
/// Each level above B₁ adds roughly an order of magnitude of overhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AbstractionLevel {
    /// B₀ — physical substrate (analog, memristor, photonic).
    B0Physics,
    /// B₁ — hardware gate (transistor, dedicated functional unit).
    B1Hardware,
    /// B₂ — microarchitecture (shader core, tensor core, ALU pipeline).
    B2Microarch,
    /// B₃ — ISA instruction (assembly, intrinsic, WGSL/SPIR-V op).
    B3Isa,
    /// B₄ — interpreted software (Python, Rust without intrinsics, JS).
    B4Software,
}

impl AbstractionLevel {
    /// Overhead factor relative to B₁ (hardware native), expressed as orders
    /// of magnitude. Each additional level adds roughly one decimal order.
    pub fn overhead_orders(&self) -> f64 {
        match self {
            Self::B0Physics => -0.5, // sub-hardware, free in principle
            Self::B1Hardware => 0.0,
            Self::B2Microarch => 1.0,
            Self::B3Isa => 2.0,
            Self::B4Software => 3.5,
        }
    }
}

/// Concurrency topology — how the operation's data parallelizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConcurrencyTopology {
    /// D₁ — vector (SIMD-friendly, contiguous).
    D1Vector,
    /// D₂ — matrix (2-D structured, tensor cores).
    D2Matrix,
    /// D₃ — graph (irregular, sparse, indexed).
    D3Graph,
    /// D₄ — embarrassingly parallel (independent per-element).
    D4EmbarrassinglyParallel,
    /// D₅ — sequential (data dependence forces serial).
    D5Sequential,
}

impl ConcurrencyTopology {
    /// Mismatch penalty in orders of magnitude when this topology is forced
    /// onto hardware that expects a different one.
    pub fn mismatch_penalty(&self, natural: ConcurrencyTopology) -> f64 {
        if *self == natural {
            return 0.0;
        }
        // Forcing parallel onto sequential is the worst case
        match (*self, natural) {
            (Self::D5Sequential, Self::D4EmbarrassinglyParallel) => 3.0,
            (Self::D5Sequential, Self::D2Matrix) => 2.5,
            (Self::D5Sequential, _) => 2.0,
            (Self::D4EmbarrassinglyParallel, Self::D5Sequential) => 0.5,
            // Adjacent topologies cost less to translate
            _ => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Operation registry
// ---------------------------------------------------------------------------

/// A registered operation in cad-future. Captures the *current* implementation
/// coordinates so the analyzer can compare to the natural coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Human-readable name (e.g. `"fea_stiffness_assembly"`, `"tessellate_face"`).
    pub name: String,
    /// What primitive this operation IS, structurally.
    pub primitive: Primitive,
    /// Where it currently runs.
    pub abstraction: AbstractionLevel,
    /// How it currently parallelizes.
    pub topology: ConcurrencyTopology,
    /// What it does to information. Almost always equal to `primitive.info_family()`.
    pub info_topology: InformationTopology,
}

/// Output of [`analyze`] — the impedance report for a single operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpedanceReport {
    pub operation: String,
    /// Thermodynamic floor (the cheapest this op can possibly be).
    pub floor: LandauerLevel,
    /// Current cost in orders of magnitude above the floor.
    pub current_orders_above_floor: f64,
    /// Cost achievable by moving to the natural coordinate.
    pub natural_orders_above_floor: f64,
    /// Gap in orders of magnitude (current − natural). > 0 means waste.
    pub gap_orders: f64,
    /// Estimated speedup factor if the gap is closed (10^gap_orders).
    pub estimated_speedup: f64,
    /// Concrete fix recommendations.
    pub recommendations: Vec<String>,
}

/// Analyze a single operation and report its impedance gap.
pub fn analyze(op: &Operation) -> ImpedanceReport {
    let floor = op.primitive.info_family().floor();

    let current_orders = op.abstraction.overhead_orders()
        + op.topology.mismatch_penalty(op.primitive.natural_topology());

    let natural_orders = op.primitive.natural_abstraction().overhead_orders();
    // Natural topology has zero mismatch penalty by definition.

    let gap = (current_orders - natural_orders).max(0.0);
    let speedup = 10.0_f64.powf(gap);

    let mut recommendations = Vec::new();

    if op.abstraction > op.primitive.natural_abstraction() {
        recommendations.push(format!(
            "Move from {:?} to {:?} (gap: {} orders of magnitude)",
            op.abstraction,
            op.primitive.natural_abstraction(),
            (op.abstraction.overhead_orders() - op.primitive.natural_abstraction().overhead_orders()) as i64,
        ));
    }

    if op.topology != op.primitive.natural_topology() {
        recommendations.push(format!(
            "Reshape work from {:?} to {:?} concurrency (penalty: {:.1} orders)",
            op.topology,
            op.primitive.natural_topology(),
            op.topology.mismatch_penalty(op.primitive.natural_topology()),
        ));
    }

    if op.primitive.info_family() == InformationTopology::Bijective
        && op.abstraction == AbstractionLevel::B4Software
    {
        recommendations.push(
            "This is a bijective (L₀) operation paying software overhead — \
             cache the result as a LUT and read it forever for free."
                .to_string(),
        );
    }

    if recommendations.is_empty() {
        recommendations.push(
            "No impedance gap detected — operation is already at natural coordinates.".into(),
        );
    }

    ImpedanceReport {
        operation: op.name.clone(),
        floor,
        current_orders_above_floor: current_orders + floor.cost_order(),
        natural_orders_above_floor: natural_orders + floor.cost_order(),
        gap_orders: gap,
        estimated_speedup: speedup,
        recommendations,
    }
}

// ---------------------------------------------------------------------------
// Registry — analyze all of cad-future at once
// ---------------------------------------------------------------------------

/// Audit a batch of operations and return reports sorted by largest gap first.
pub fn audit(ops: &[Operation]) -> Vec<ImpedanceReport> {
    let mut reports: Vec<ImpedanceReport> = ops.iter().map(analyze).collect();
    reports.sort_by(|a, b| b.gap_orders.partial_cmp(&a.gap_orders).unwrap());
    reports
}

/// Total estimated speedup if every gap in the audit were closed.
/// Geometric mean of individual speedups (so it doesn't double-count
/// correlated wins).
pub fn audit_total_speedup(ops: &[Operation]) -> f64 {
    if ops.is_empty() {
        return 1.0;
    }
    let total_gap: f64 = ops
        .iter()
        .map(|op| analyze(op).gap_orders)
        .sum::<f64>()
        / ops.len() as f64;
    10.0_f64.powf(total_gap)
}

// ---------------------------------------------------------------------------
// Type-level info-topology vocabulary
// ---------------------------------------------------------------------------
//
// The impedance framework above describes information topology at *runtime*
// (via the `InformationTopology` enum) and is checked only when an operation
// is explicitly registered with `analyze()`. That leaves a gap: nothing
// prevents a programmer from writing a function whose structure is reductive
// but whose registry entry claims it's bijective (or vice versa).
//
// The traits below promote the distinction to the *type system* so the
// compiler can enforce it. A type that implements `Bijective` has to provide
// both `forward` and `inverse`, with a round-trip identity guaranteed by a
// property test (`check_bijective_roundtrip`). A type that implements
// `Reductive` only provides `reduce` — no inverse exists, by definition.
//
// On top of `Bijective` we build `Cacheable`, which encodes "this function is
// pure and its output is fully determined by a cheap content key". Any type
// with a `Cacheable` impl can be wrapped in the generic `Cache<C>` LUT, which
// unifies the three hand-rolled caches from the April-2026 sprint
// (tessellation, point-in-solid, depth render) under one abstraction.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;

/// A bijective (information-preserving) operation.
///
/// Implementing this trait is a **type-level claim** that `forward` and
/// `inverse` together form a round-trip identity:
///
/// ```text
/// inverse(forward(a)) == a   for all a in A
/// forward(inverse(b)) == b   for all b in B
/// ```
///
/// Thermodynamically, any operation satisfying this contract sits at
/// `LandauerLevel::L0Free` — it erases no information and therefore pays no
/// Landauer cost in principle. Use [`check_bijective_roundtrip`] in tests
/// to verify the identity holds for your implementation on sample inputs.
pub trait Bijective {
    type A;
    type B;
    fn forward(&self, a: Self::A) -> Self::B;
    fn inverse(&self, b: Self::B) -> Self::A;
}

/// A reductive operation: `In → Out` with no inverse.
///
/// The type signature itself carries the information-topology claim — there
/// is no way to round-trip through a `Reductive` impl, so the compiler
/// cannot be tricked into caching a reductive op as if it were bijective.
pub trait Reductive {
    type In;
    type Out;
    fn reduce(&self, input: Self::In) -> Self::Out;
}

/// Verify that a `Bijective` implementation round-trips on a given input.
///
/// Returns `true` iff `inverse(forward(a)) == a`. Call this from a property
/// test for every `Bijective` impl — it's the only way to catch a broken
/// inverse at compile-ish time (the compiler checks the signature, this
/// function checks the law).
pub fn check_bijective_roundtrip<B>(op: &B, sample: B::A) -> bool
where
    B: Bijective,
    B::A: Clone + PartialEq,
    B::B: Clone,
{
    let a0 = sample.clone();
    let b = op.forward(sample);
    let a1 = op.inverse(b);
    a0 == a1
}

/// A content-addressable pure function.
///
/// Implementors commit to: given identical `Key`s, the `compute()` output is
/// identical. This is the minimal contract for sound LUT caching — weaker
/// than full `Bijective` because the input-space `Input` may be larger than
/// the key-space `Key`, but the key must be a total function of the input
/// that determines the output.
///
/// The generic [`Cache<C>`] uses this trait to provide `get_or_compute`
/// semantics backed by a `HashMap<Key, Output>`.
pub trait Cacheable {
    type Input: ?Sized;
    type Key: Eq + Hash + Clone;
    type Output: Clone;

    /// Compute the cache key for an input. Must be a **total function of
    /// content**: two inputs whose `compute()` outputs differ must hash to
    /// different keys.
    fn key(input: &Self::Input) -> Self::Key;

    /// Compute the output from an input. Must be **pure**: depend only on
    /// `input` (no hidden state, no wall-clock, no randomness).
    fn compute(input: &Self::Input) -> Self::Output;
}

/// Snapshot of cache occupancy and hit/miss counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
}

/// Generic LUT cache backed by `HashMap<C::Key, C::Output>`.
///
/// Replaces the hand-rolled `TessCache` / `AccelCache` / `DepthCache`
/// structures with a single type-safe abstraction. The `C: Cacheable`
/// bound is a type-level statement that the wrapped function is pure and
/// its output is fully determined by `C::Key` — without that guarantee,
/// caching is unsound.
pub struct Cache<C: Cacheable> {
    map: Mutex<HashMap<C::Key, C::Output>>,
    hits: Mutex<u64>,
    misses: Mutex<u64>,
    _phantom: std::marker::PhantomData<fn(&C::Input) -> C::Output>,
}

impl<C: Cacheable> Default for Cache<C> {
    fn default() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            hits: Mutex::new(0),
            misses: Mutex::new(0),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C: Cacheable> Cache<C> {
    pub fn new() -> Self { Self::default() }

    /// Fetch (or compute and insert) the output for `input`.
    pub fn get_or_compute(&self, input: &C::Input) -> C::Output {
        let k = C::key(input);
        {
            let map = self.map.lock().unwrap();
            if let Some(v) = map.get(&k) {
                *self.hits.lock().unwrap() += 1;
                return v.clone();
            }
        }
        let v = C::compute(input);
        let mut map = self.map.lock().unwrap();
        // Double-check: another thread may have populated the key while we
        // were computing. We accept our own value over theirs (same output
        // by the purity contract) to avoid a second `compute` call.
        map.entry(k).or_insert_with(|| v.clone());
        *self.misses.lock().unwrap() += 1;
        v
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.map.lock().unwrap().len(),
            hits: *self.hits.lock().unwrap(),
            misses: *self.misses.lock().unwrap(),
        }
    }

    pub fn clear(&self) {
        self.map.lock().unwrap().clear();
        *self.hits.lock().unwrap() = 0;
        *self.misses.lock().unwrap() = 0;
    }
}

// ---------------------------------------------------------------------------
// Standard cad-future operations — pre-registered with their current coordinates
// ---------------------------------------------------------------------------

/// The current coordinates of every measured cad-future hot path.
/// This is the input to `audit()` — update entries here as crates evolve.
pub fn cad_future_operations() -> Vec<Operation> {
    vec![
        // Assembly now runs through `physical_fea::sparse::assemble_stiffness_sparse`:
        // rayon-parallel per-element stiffness computation into CSR triplets.
        // The public `solve()` entry point dispatches problems above
        // `SPARSE_DOF_THRESHOLD` DOFs to the sparse path automatically, so
        // every non-trivial FEA workload hits this. Structurally it is a
        // graph-shaped (D3) sparse matmul at ISA level — its natural
        // coordinates.
        Operation {
            name: "fea_stiffness_assembly".into(),
            primitive: Primitive::SparseMatmul,
            abstraction: AbstractionLevel::B3Isa,
            topology: ConcurrencyTopology::D3Graph,
            info_topology: InformationTopology::Reductive,
        },
        Operation {
            name: "fea_solve_pcg".into(),
            primitive: Primitive::SparseMatmul,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Reductive,
        },
        // Tessellation is now backed by a content-addressed LUT cache
        // (`physical-tessellation::tessellate`). Repeat calls are reduced to
        // hash + Arc::clone — a handful of ISA-level ops, embarrassingly parallel
        // across distinct solids. On a cache miss the underlying compute still
        // runs, but the amortised path is dominated by the hit case.
        Operation {
            name: "tessellate_face_uv_grid".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B3Isa,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        },
        // Point-in-solid is now backed by `AccelCache` + per-face AABB filter
        // (`physical-brep::point_in_solid`). A query amortises to a hash lookup
        // plus an O(candidates) pruned walk rather than an O(F) full scan.
        Operation {
            name: "boolean_point_in_solid".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B3Isa,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        },
        // Jacobian construction is rayon-parallel per column and J^T·J is
        // rayon-parallel per row (`physical_sketch::solver::build_jacobian`,
        // `mat_mul_transpose`). Column-wise finite differences and inner
        // products are independent, so this is D4 embarrassingly parallel
        // float arithmetic at B3Isa — its natural coordinates.
        Operation {
            name: "sketch_solver_jacobian".into(),
            primitive: Primitive::FloatArith,
            abstraction: AbstractionLevel::B3Isa,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Reductive,
        },
        Operation {
            name: "lut_material_lookup".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        },
        Operation {
            name: "cfl_lex_tokenize".into(),
            primitive: Primitive::FiniteAutomaton,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Reductive,
        },
        Operation {
            name: "cfl_parse_ast".into(),
            primitive: Primitive::FiniteAutomaton,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Reductive,
        },
        Operation {
            name: "agent_cascade_query".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        },
        Operation {
            name: "surrogate_predict_structural".into(),
            primitive: Primitive::FloatArith,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Reductive,
        },
        Operation {
            name: "scan2cad_ransac_plane".into(),
            primitive: Primitive::MonteCarlo,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Reductive,
        },
        // Region-growing now precomputes normals/centroids/areas in
        // parallel (`physical_inverse::segment_mesh`) and uses a dense
        // per-triangle adjacency list. The flood fill itself is still
        // sequential (inherent data-dep), but its inner loop is now a
        // dense-array read + dot product — D1-vector access. Region
        // classification is rayon-parallel across regions.
        Operation {
            name: "inverse_segment_region_grow".into(),
            primitive: Primitive::Reduction,
            abstraction: AbstractionLevel::B3Isa,
            topology: ConcurrencyTopology::D1Vector,
            info_topology: InformationTopology::Reductive,
        },
        // Depth rasterisation is now backed by `DepthCache`
        // (`physical-gen-bridge::render_depth`). Repeat renders of the same
        // (mesh, view) key — the common case during diffusion conditioning —
        // reduce to a hash-table fetch and an Arc clone.
        Operation {
            name: "gen_bridge_render_depth".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B3Isa,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        },
        Operation {
            name: "em_fdtd_2d_step".into(),
            primitive: Primitive::Convolution,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D2Matrix,
            info_topology: InformationTopology::Reductive,
        },
    ]
}

// ---------------------------------------------------------------------------
// Pretty-print summary for the CLI / web frontend
// ---------------------------------------------------------------------------

/// Render a single report as a multi-line summary.
pub fn format_report(report: &ImpedanceReport) -> String {
    let mut s = String::new();
    s.push_str(&format!("operation: {}\n", report.operation));
    s.push_str(&format!("  floor:           {:?}\n", report.floor));
    s.push_str(&format!(
        "  current cost:    {:.1} orders above floor\n",
        report.current_orders_above_floor
    ));
    s.push_str(&format!(
        "  natural cost:    {:.1} orders above floor\n",
        report.natural_orders_above_floor
    ));
    s.push_str(&format!(
        "  impedance gap:   {:.1} orders   →   ~{:.0}× speedup\n",
        report.gap_orders, report.estimated_speedup
    ));
    s.push_str("  recommendations:\n");
    for rec in &report.recommendations {
        s.push_str(&format!("    • {}\n", rec));
    }
    s
}

/// Render a full audit table.
pub fn format_audit(reports: &[ImpedanceReport]) -> String {
    let mut s = String::new();
    s.push_str("                                   gap     est.\n");
    s.push_str("operation                          orders  speedup\n");
    s.push_str("------------------------------------------------------\n");
    for r in reports {
        s.push_str(&format!(
            "{:<33}  {:>5.1}   {:>6.0}×\n",
            r.operation, r.gap_orders, r.estimated_speedup
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bijective_primitives_have_l0_floor() {
        for p in [
            Primitive::BitwiseLogic,
            Primitive::Fft,
            Primitive::ScatterGather,
            Primitive::EmbeddingLookup,
            Primitive::Wavelet,
            Primitive::NttTransform,
        ] {
            assert_eq!(p.info_family().floor(), LandauerLevel::L0Free,
                "{:?} should be L₀", p);
        }
    }

    #[test]
    fn reductive_primitives_have_l2_floor() {
        for p in [
            Primitive::DenseMatmul,
            Primitive::Softmax,
            Primitive::ComparisonPredicate,
            Primitive::Reduction,
            Primitive::Activation,
        ] {
            assert_eq!(p.info_family().floor(), LandauerLevel::L2Landauer,
                "{:?} should be L₂", p);
        }
    }

    #[test]
    fn rng_is_l1_stochastic() {
        assert_eq!(Primitive::Rng.info_family(), InformationTopology::Stochastic);
        assert_eq!(Primitive::Rng.info_family().floor(), LandauerLevel::L1Entropy);
    }

    #[test]
    fn primitive_ids_match_periodic_stack() {
        // Spot-check a few from the source notes
        assert_eq!(Primitive::BitwiseLogic.id(), 2);
        assert_eq!(Primitive::DenseMatmul.id(), 18);
        assert_eq!(Primitive::Reduction.id(), 21);
        assert_eq!(Primitive::Fft.id(), 23);
        assert_eq!(Primitive::Convolution.id(), 24);
        assert_eq!(Primitive::ScatterGather.id(), 40);
        assert_eq!(Primitive::EmbeddingLookup.id(), 41);
        assert_eq!(Primitive::FiniteAutomaton.id(), 42);
        assert_eq!(Primitive::RelationalJoin.id(), 43);
    }

    #[test]
    fn b4_software_has_more_overhead_than_b1() {
        assert!(
            AbstractionLevel::B4Software.overhead_orders()
                > AbstractionLevel::B1Hardware.overhead_orders()
        );
    }

    #[test]
    fn sequential_on_parallel_workload_is_penalised() {
        let penalty = ConcurrencyTopology::D5Sequential
            .mismatch_penalty(ConcurrencyTopology::D4EmbarrassinglyParallel);
        assert!(penalty >= 2.5, "expected significant penalty, got {}", penalty);
    }

    #[test]
    fn matched_topology_has_zero_penalty() {
        for t in [
            ConcurrencyTopology::D1Vector,
            ConcurrencyTopology::D2Matrix,
            ConcurrencyTopology::D4EmbarrassinglyParallel,
        ] {
            assert_eq!(t.mismatch_penalty(t), 0.0);
        }
    }

    #[test]
    fn analyze_l0_op_at_b4_reports_large_gap() {
        // Tessellation: bijective scatter/gather running in software
        let op = Operation {
            name: "tessellate_face_uv_grid".into(),
            primitive: Primitive::ScatterGather,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Bijective,
        };
        let report = analyze(&op);
        assert!(report.gap_orders > 3.0,
            "L₀ op at B₄ should have >3 orders gap, got {}", report.gap_orders);
        assert!(report.estimated_speedup > 100.0);
    }

    #[test]
    fn analyze_already_natural_has_no_gap() {
        let op = Operation {
            name: "embedding_lookup_lut".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B1Hardware,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        };
        let report = analyze(&op);
        assert!(report.gap_orders < 0.5, "already-optimal op has gap {}", report.gap_orders);
    }

    #[test]
    fn analyze_recommends_lut_for_l0_at_b4() {
        let op = Operation {
            name: "boolean_point_in_solid".into(),
            primitive: Primitive::EmbeddingLookup,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Bijective,
        };
        let report = analyze(&op);
        assert!(
            report.recommendations.iter().any(|r| r.contains("LUT")),
            "L₀ at B₄ should recommend LUT caching, got: {:?}",
            report.recommendations
        );
    }

    #[test]
    fn audit_sorts_largest_gap_first() {
        let ops = cad_future_operations();
        let reports = audit(&ops);
        for w in reports.windows(2) {
            assert!(w[0].gap_orders >= w[1].gap_orders);
        }
    }

    #[test]
    fn audit_finds_real_cad_future_waste() {
        let ops = cad_future_operations();
        let reports = audit(&ops);
        // Tessellation, point-in-solid, FEA assembly should all be in the
        // top half — they're known impedance offenders.
        let top_names: Vec<&str> = reports
            .iter()
            .take(reports.len() / 2 + 1)
            .map(|r| r.operation.as_str())
            .collect();
        assert!(
            top_names.contains(&"tessellate_face_uv_grid")
                || top_names.contains(&"boolean_point_in_solid")
                || top_names.contains(&"fea_stiffness_assembly"),
            "expected a known offender in top half, got: {:?}",
            top_names
        );
    }

    #[test]
    fn audit_total_speedup_above_one_when_waste_exists() {
        let ops = cad_future_operations();
        let speedup = audit_total_speedup(&ops);
        assert!(speedup > 1.0, "audit should detect cumulative waste, got {}", speedup);
    }

    #[test]
    fn format_audit_produces_table() {
        let ops = cad_future_operations();
        let reports = audit(&ops);
        let formatted = format_audit(&reports);
        assert!(formatted.contains("operation"));
        assert!(formatted.contains("speedup"));
        // Every operation should appear in the table
        for op in &ops {
            assert!(formatted.contains(&op.name), "missing {} from output", op.name);
        }
    }

    #[test]
    fn format_report_includes_floor_and_recs() {
        let op = &cad_future_operations()[0];
        let report = analyze(op);
        let formatted = format_report(&report);
        assert!(formatted.contains("floor"));
        assert!(formatted.contains("recommendations"));
        assert!(formatted.contains(&op.name));
    }

    #[test]
    fn fea_assembly_predicted_at_least_two_orders_of_speedup() {
        let op = Operation {
            name: "fea_stiffness_assembly".into(),
            primitive: Primitive::DenseMatmul,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Reductive,
        };
        let report = analyze(&op);
        // B4 + D5 sequential on a D2 matrix op should be 2-3 orders away
        assert!(
            report.gap_orders >= 2.0,
            "FEA assembly gap {} should be ≥ 2 orders",
            report.gap_orders
        );
    }

    #[test]
    fn rng_floor_is_l1_not_l0_or_l2() {
        let op = Operation {
            name: "scan2cad_ransac_plane".into(),
            primitive: Primitive::Rng,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Stochastic,
        };
        let report = analyze(&op);
        assert_eq!(report.floor, LandauerLevel::L1Entropy);
    }

    #[test]
    fn cad_future_operations_registry_is_non_empty() {
        let ops = cad_future_operations();
        assert!(ops.len() >= 10, "expected ≥10 registered hot paths, got {}", ops.len());
    }

    #[test]
    fn natural_topology_known_for_every_primitive() {
        // Smoke test: every primitive variant returns a topology without panic
        for p in [
            Primitive::BitwiseLogic, Primitive::DenseMatmul, Primitive::SparseMatmul,
            Primitive::ScatterGather, Primitive::EmbeddingLookup, Primitive::Fft,
            Primitive::Convolution, Primitive::Softmax, Primitive::Activation,
            Primitive::Rng, Primitive::MonteCarlo, Primitive::FiniteAutomaton,
        ] {
            let _ = p.natural_topology();
            let _ = p.natural_abstraction();
        }
    }

    // ---------------------------------------------------------------
    // Phase C: type-level info-topology enforcement
    // ---------------------------------------------------------------

    /// Toy bijective op — rotate a `(x, y)` pair by 90°. `forward` rotates
    /// CCW, `inverse` rotates CW.
    struct Rot90;
    impl Bijective for Rot90 {
        type A = (i32, i32);
        type B = (i32, i32);
        fn forward(&self, a: (i32, i32)) -> (i32, i32) { (-a.1, a.0) }
        fn inverse(&self, b: (i32, i32)) -> (i32, i32) { (b.1, -b.0) }
    }

    /// Toy reductive op — sum an array. There is no inverse: given the
    /// sum, the original array cannot be recovered.
    struct SumReduce;
    impl Reductive for SumReduce {
        type In = Vec<i32>;
        type Out = i32;
        fn reduce(&self, input: Vec<i32>) -> i32 { input.iter().sum() }
    }

    #[test]
    fn bijective_roundtrip_holds_for_rot90() {
        let op = Rot90;
        for a in [(0, 0), (1, 0), (3, -2), (-7, 5), (i32::MAX / 2, i32::MIN / 2)] {
            assert!(
                check_bijective_roundtrip(&op, a),
                "rot90 round-trip failed for {:?}",
                a
            );
        }
    }

    #[test]
    fn reductive_op_reduces() {
        let op = SumReduce;
        assert_eq!(op.reduce(vec![]), 0);
        assert_eq!(op.reduce(vec![1, 2, 3, 4]), 10);
    }

    /// Toy cacheable op — square an integer. Key is the integer itself
    /// (trivial content hash); compute is `n * n`.
    struct SquareLookup;
    impl Cacheable for SquareLookup {
        type Input = i32;
        type Key = i32;
        type Output = i64;
        fn key(input: &i32) -> i32 { *input }
        fn compute(input: &i32) -> i64 { (*input as i64) * (*input as i64) }
    }

    #[test]
    fn generic_cache_returns_correct_values() {
        let cache: Cache<SquareLookup> = Cache::new();
        assert_eq!(cache.get_or_compute(&5), 25);
        assert_eq!(cache.get_or_compute(&-7), 49);
        assert_eq!(cache.get_or_compute(&0), 0);
    }

    #[test]
    fn generic_cache_hits_on_repeat_queries() {
        let cache: Cache<SquareLookup> = Cache::new();
        for _ in 0..10 {
            let _ = cache.get_or_compute(&42);
        }
        let s = cache.stats();
        assert_eq!(s.misses, 1, "only first call should miss");
        assert_eq!(s.hits, 9, "subsequent 9 calls should all hit");
        assert_eq!(s.entries, 1);
    }

    #[test]
    fn generic_cache_distinct_keys_grow_entries() {
        let cache: Cache<SquareLookup> = Cache::new();
        for i in 0..5 {
            let _ = cache.get_or_compute(&i);
        }
        assert_eq!(cache.stats().entries, 5);
    }

    /// Demonstrates `Cacheable` over an unsized input (`[i32]`). This is
    /// the shape real tessellation/point-in-solid caches take: the
    /// underlying data is borrowed, the key is a cheap content hash, and
    /// the compute function is pure.
    struct SortSlice;
    impl Cacheable for SortSlice {
        type Input = [i32];
        type Key = u64;
        type Output = Vec<i32>;
        fn key(input: &[i32]) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::Hasher;
            let mut h = DefaultHasher::new();
            input.hash(&mut h);
            h.finish()
        }
        fn compute(input: &[i32]) -> Vec<i32> {
            let mut v = input.to_vec();
            v.sort();
            v
        }
    }

    #[test]
    fn generic_cache_works_with_unsized_input() {
        let cache: Cache<SortSlice> = Cache::new();
        let a = [3, 1, 4, 1, 5, 9, 2, 6];
        let b = [2, 7, 1, 8];

        assert_eq!(cache.get_or_compute(&a[..]), vec![1, 1, 2, 3, 4, 5, 6, 9]);
        assert_eq!(cache.get_or_compute(&a[..]), vec![1, 1, 2, 3, 4, 5, 6, 9]);
        assert_eq!(cache.get_or_compute(&b[..]), vec![1, 2, 7, 8]);

        let s = cache.stats();
        assert_eq!(s.misses, 2, "first query of each distinct input misses");
        assert_eq!(s.hits, 1, "repeat query of `a` hits");
        assert_eq!(s.entries, 2);
    }

    #[test]
    fn generic_cache_clear_resets_stats() {
        let cache: Cache<SquareLookup> = Cache::new();
        let _ = cache.get_or_compute(&1);
        let _ = cache.get_or_compute(&1);
        cache.clear();
        let s = cache.stats();
        assert_eq!(s.entries, 0);
        assert_eq!(s.hits, 0);
        assert_eq!(s.misses, 0);
    }

    #[test]
    fn closing_gap_yields_predicted_speedup() {
        // Before: tessellation at B4 sequential
        let before = Operation {
            name: "tess_before".into(),
            primitive: Primitive::ScatterGather,
            abstraction: AbstractionLevel::B4Software,
            topology: ConcurrencyTopology::D5Sequential,
            info_topology: InformationTopology::Bijective,
        };
        // After: same primitive at B2 microarch with parallel topology
        let after = Operation {
            name: "tess_after".into(),
            primitive: Primitive::ScatterGather,
            abstraction: AbstractionLevel::B2Microarch,
            topology: ConcurrencyTopology::D4EmbarrassinglyParallel,
            info_topology: InformationTopology::Bijective,
        };

        let r_before = analyze(&before);
        let r_after = analyze(&after);

        assert!(r_before.gap_orders > r_after.gap_orders);
        assert!(r_before.estimated_speedup > r_after.estimated_speedup);
    }
}
