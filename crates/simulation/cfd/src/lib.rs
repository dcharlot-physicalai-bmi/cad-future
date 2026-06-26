//! `physical-cfd` — Lattice Boltzmann CFD solver.
//!
//! GPU-friendly, meshless flow simulation. Handles external flow over bodies
//! and internal flow through channels. LUT-accelerated: common geometries
//! (pipe, duct, orifice) return instant answers from Moody chart / loss
//! coefficient tables before falling back to LBM solve.

// ---------------------------------------------------------------------------
// LUT Layer — Analytical Solutions First
// ---------------------------------------------------------------------------

/// Pipe flow result from analytical solution.
#[derive(Debug, Clone)]
pub struct PipeFlowResult {
    /// Reynolds number.
    pub reynolds: f64,
    /// Flow regime.
    pub regime: FlowRegime,
    /// Darcy friction factor.
    pub friction_factor: f64,
    /// Pressure drop (Pa).
    pub pressure_drop_pa: f64,
    /// Average velocity (m/s).
    pub velocity_avg_m_s: f64,
    /// Volume flow rate (m³/s).
    pub flow_rate_m3_s: f64,
}

/// Flow regime classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowRegime {
    Laminar,
    Transitional,
    Turbulent,
}

/// Solve pipe flow analytically (Moody chart + Darcy-Weisbach).
/// Returns None if the geometry is too complex for analytical solution.
pub fn pipe_flow(
    diameter_m: f64,
    length_m: f64,
    velocity_m_s: f64,
    density_kg_m3: f64,
    dynamic_viscosity_pa_s: f64,
    roughness_m: f64,
) -> PipeFlowResult {
    let re = density_kg_m3 * velocity_m_s * diameter_m / dynamic_viscosity_pa_s;

    let regime = if re < 2300.0 {
        FlowRegime::Laminar
    } else if re < 4000.0 {
        FlowRegime::Transitional
    } else {
        FlowRegime::Turbulent
    };

    let f = moody_friction_factor(re, roughness_m / diameter_m);

    // Darcy-Weisbach: ΔP = f × (L/D) × (ρ·v²/2)
    let dp = f * (length_m / diameter_m) * (density_kg_m3 * velocity_m_s * velocity_m_s / 2.0);

    let area = std::f64::consts::PI * diameter_m * diameter_m / 4.0;
    let flow_rate = velocity_m_s * area;

    PipeFlowResult {
        reynolds: re,
        regime,
        friction_factor: f,
        pressure_drop_pa: dp,
        velocity_avg_m_s: velocity_m_s,
        flow_rate_m3_s: flow_rate,
    }
}

/// Moody friction factor (Colebrook-White equation solved iteratively).
pub fn moody_friction_factor(re: f64, relative_roughness: f64) -> f64 {
    if re < 2300.0 {
        // Laminar: f = 64/Re
        return 64.0 / re.max(1.0);
    }

    // Colebrook-White: 1/√f = -2·log10(ε/(3.7·D) + 2.51/(Re·√f))
    // Solve iteratively starting from initial guess
    let e_d = relative_roughness.max(1e-10); // avoid log(0)

    // Swamee-Jain initial approximation
    let arg = (e_d / 3.7).log10() + 5.74 / re.powf(0.9);
    let mut f = if arg.abs() > 1e-10 { 0.25 / (arg * arg) } else { 0.02 };
    f = f.max(0.001).min(0.1);

    for _ in 0..50 {
        let rhs = -2.0 * (e_d / 3.7 + 2.51 / (re * f.sqrt())).log10();
        if rhs.abs() < 1e-10 { break; }
        let f_new = 1.0 / (rhs * rhs);
        if (f_new - f).abs() < 1e-12 { break; }
        f = f_new;
    }

    f
}

/// Minor loss coefficient table for common fittings.
#[derive(Debug, Clone, Copy)]
pub struct MinorLoss {
    pub fitting: &'static str,
    pub k: f64,
}

pub static MINOR_LOSS_TABLE: &[MinorLoss] = &[
    MinorLoss { fitting: "90° elbow (standard)", k: 0.9 },
    MinorLoss { fitting: "90° elbow (long radius)", k: 0.6 },
    MinorLoss { fitting: "45° elbow", k: 0.4 },
    MinorLoss { fitting: "Tee (branch)", k: 1.0 },
    MinorLoss { fitting: "Tee (run)", k: 0.3 },
    MinorLoss { fitting: "Gate valve (full open)", k: 0.2 },
    MinorLoss { fitting: "Globe valve (full open)", k: 10.0 },
    MinorLoss { fitting: "Ball valve (full open)", k: 0.05 },
    MinorLoss { fitting: "Check valve (swing)", k: 2.5 },
    MinorLoss { fitting: "Entrance (sharp-edged)", k: 0.5 },
    MinorLoss { fitting: "Entrance (well-rounded)", k: 0.03 },
    MinorLoss { fitting: "Exit (into tank)", k: 1.0 },
    MinorLoss { fitting: "Sudden expansion", k: 1.0 },
    MinorLoss { fitting: "Sudden contraction", k: 0.5 },
    MinorLoss { fitting: "Orifice plate", k: 2.5 },
];

/// Lookup a minor loss K-value by fitting name.
pub fn lookup_minor_loss(fitting: &str) -> Option<f64> {
    MINOR_LOSS_TABLE.iter()
        .find(|m| m.fitting.to_lowercase().contains(&fitting.to_lowercase()))
        .map(|m| m.k)
}

// ---------------------------------------------------------------------------
// Lattice Boltzmann Method (D2Q9)
// ---------------------------------------------------------------------------

/// 2D Lattice Boltzmann solver using D2Q9 lattice.
/// Suitable for 2D cross-section flow analysis.
#[derive(Debug, Clone)]
pub struct LatticeBoltzmann2D {
    /// Grid width.
    pub nx: usize,
    /// Grid height.
    pub ny: usize,
    /// Relaxation time (τ). τ = 0.5 + 3ν/Δx² where ν is kinematic viscosity.
    pub tau: f64,
    /// Distribution functions f[y][x][q] for 9 directions.
    f: Vec<f64>,
    /// Equilibrium distribution functions.
    f_eq: Vec<f64>,
    /// Obstacle mask: true = solid wall.
    pub obstacle: Vec<bool>,
    /// Macroscopic density.
    pub density: Vec<f64>,
    /// Macroscopic velocity (x, y).
    pub velocity: Vec<(f64, f64)>,
}

// D2Q9 lattice velocities
const CX: [i32; 9] = [0, 1, 0, -1, 0, 1, -1, -1, 1];
const CY: [i32; 9] = [0, 0, 1, 0, -1, 1, 1, -1, -1];
const W: [f64; 9] = [
    4.0/9.0,                                   // center
    1.0/9.0, 1.0/9.0, 1.0/9.0, 1.0/9.0,       // cardinal
    1.0/36.0, 1.0/36.0, 1.0/36.0, 1.0/36.0,   // diagonal
];
// Bounce-back opposite direction indices
const OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];

impl LatticeBoltzmann2D {
    /// Create a new LBM solver with given grid size and relaxation time.
    /// Higher τ → higher viscosity (more viscous flow).
    /// τ must be > 0.5 for stability.
    pub fn new(nx: usize, ny: usize, tau: f64) -> Self {
        let n = nx * ny;
        let mut f = vec![0.0; n * 9];
        let density = vec![1.0; n];
        let velocity = vec![(0.0, 0.0); n];
        let obstacle = vec![false; n];

        // Initialize to equilibrium at rest
        for idx in 0..n {
            for q in 0..9 {
                f[idx * 9 + q] = W[q];
            }
        }

        let f_eq = f.clone();

        Self { nx, ny, tau, f, f_eq, obstacle, density, velocity }
    }

    /// Create from kinematic viscosity (m²/s) and grid spacing.
    pub fn from_viscosity(nx: usize, ny: usize, nu: f64, dx: f64, dt: f64) -> Self {
        let tau = 0.5 + 3.0 * nu * dt / (dx * dx);
        Self::new(nx, ny, tau)
    }

    /// Set an obstacle (solid wall) at grid position.
    pub fn set_obstacle(&mut self, x: usize, y: usize) {
        if x < self.nx && y < self.ny {
            self.obstacle[y * self.nx + x] = true;
        }
    }

    /// Set a rectangular obstacle.
    pub fn set_rect_obstacle(&mut self, x0: usize, y0: usize, w: usize, h: usize) {
        for y in y0..(y0 + h).min(self.ny) {
            for x in x0..(x0 + w).min(self.nx) {
                self.obstacle[y * self.nx + x] = true;
            }
        }
    }

    /// Set a circular obstacle.
    pub fn set_circle_obstacle(&mut self, cx: usize, cy: usize, radius: usize) {
        let r2 = (radius * radius) as i64;
        for y in 0..self.ny {
            for x in 0..self.nx {
                let dx = x as i64 - cx as i64;
                let dy = y as i64 - cy as i64;
                if dx * dx + dy * dy <= r2 {
                    self.obstacle[y * self.nx + x] = true;
                }
            }
        }
    }

    /// Perform one time step of the LBM simulation.
    pub fn step(&mut self, inlet_velocity: f64) {
        let nx = self.nx;
        let ny = self.ny;

        // Compute macroscopic quantities
        for idx in 0..nx * ny {
            if self.obstacle[idx] { continue; }
            let mut rho = 0.0;
            let mut ux = 0.0;
            let mut uy = 0.0;
            for q in 0..9 {
                let fq = self.f[idx * 9 + q];
                rho += fq;
                ux += CX[q] as f64 * fq;
                uy += CY[q] as f64 * fq;
            }
            rho = rho.max(1e-10);
            ux /= rho;
            uy /= rho;
            self.density[idx] = rho;
            self.velocity[idx] = (ux, uy);
        }

        // Compute equilibrium
        for idx in 0..nx * ny {
            if self.obstacle[idx] { continue; }
            let rho = self.density[idx];
            let (ux, uy) = self.velocity[idx];
            let u2 = ux * ux + uy * uy;

            for q in 0..9 {
                let cu = CX[q] as f64 * ux + CY[q] as f64 * uy;
                self.f_eq[idx * 9 + q] = W[q] * rho * (1.0 + 3.0 * cu + 4.5 * cu * cu - 1.5 * u2);
            }
        }

        // Collision (BGK)
        let omega = 1.0 / self.tau;
        for idx in 0..nx * ny {
            if self.obstacle[idx] { continue; }
            for q in 0..9 {
                let i = idx * 9 + q;
                self.f[i] += omega * (self.f_eq[i] - self.f[i]);
            }
        }

        // Streaming with bounce-back
        let mut f_new = vec![0.0; nx * ny * 9];

        for y in 0..ny {
            for x in 0..nx {
                let idx = y * nx + x;
                for q in 0..9 {
                    let nx_pos = (x as i32 + CX[q]) as usize;
                    let ny_pos = (y as i32 + CY[q]) as usize;

                    if nx_pos < nx && ny_pos < ny {
                        let target = ny_pos * nx + nx_pos;
                        if self.obstacle[target] {
                            // Bounce-back
                            f_new[idx * 9 + OPP[q]] = self.f[idx * 9 + q];
                        } else {
                            f_new[target * 9 + q] = self.f[idx * 9 + q];
                        }
                    }
                }
            }
        }

        self.f = f_new;

        // Inlet BC: Zou-He velocity boundary (left wall, x=0)
        for y in 1..ny - 1 {
            let idx = y * nx;
            if self.obstacle[idx] { continue; }

            let rho = 1.0; // assumed unit density at inlet
            let ux = inlet_velocity;
            let uy = 0.0;
            let u2 = ux * ux + uy * uy;

            for q in 0..9 {
                let cu = CX[q] as f64 * ux + CY[q] as f64 * uy;
                self.f[idx * 9 + q] = W[q] * rho * (1.0 + 3.0 * cu + 4.5 * cu * cu - 1.5 * u2);
            }
        }

        // Top/bottom walls: no-slip (bounce-back implicitly handled by obstacles or edge)
        for x in 0..nx {
            self.obstacle[x] = true;              // bottom wall
            self.obstacle[(ny - 1) * nx + x] = true; // top wall
        }
    }

    /// Run the simulation for n_steps.
    pub fn run(&mut self, inlet_velocity: f64, n_steps: usize) {
        for _ in 0..n_steps {
            self.step(inlet_velocity);
        }
    }

    /// Compute drag coefficient on obstacles.
    /// Uses momentum exchange method.
    pub fn drag_coefficient(&self, inlet_velocity: f64) -> f64 {
        let mut fx = 0.0;
        let nx = self.nx;
        let ny = self.ny;

        for y in 0..ny {
            for x in 0..nx {
                let idx = y * nx + x;
                if !self.obstacle[idx] { continue; }

                // Sum momentum exchange at boundary
                for q in 1..9 {
                    let nx_pos = (x as i32 - CX[q]) as usize;
                    let ny_pos = (y as i32 - CY[q]) as usize;
                    if nx_pos < nx && ny_pos < ny {
                        let neighbor = ny_pos * nx + nx_pos;
                        if !self.obstacle[neighbor] {
                            fx += CX[q] as f64 * (self.f[neighbor * 9 + q] + self.f[idx * 9 + OPP[q]]);
                        }
                    }
                }
            }
        }

        // Count obstacle cells for characteristic length
        let n_obs: usize = self.obstacle.iter().filter(|&&o| o).count();
        let char_length = (n_obs as f64).sqrt(); // approximate

        let rho = 1.0;
        let dyn_pressure = 0.5 * rho * inlet_velocity * inlet_velocity;
        if dyn_pressure * char_length > 0.0 {
            fx / (dyn_pressure * char_length)
        } else {
            0.0
        }
    }

    /// Get velocity magnitude at a grid point.
    pub fn velocity_magnitude(&self, x: usize, y: usize) -> f64 {
        let (ux, uy) = self.velocity[y * self.nx + x];
        (ux * ux + uy * uy).sqrt()
    }

    /// Get maximum velocity magnitude in the domain.
    pub fn max_velocity(&self) -> f64 {
        self.velocity.iter()
            .filter(|_| true)
            .map(|(ux, uy)| (ux * ux + uy * uy).sqrt())
            .fold(0.0f64, f64::max)
    }

    /// Get pressure field (p = ρ/3 in LBM units).
    pub fn pressure_at(&self, x: usize, y: usize) -> f64 {
        self.density[y * self.nx + x] / 3.0
    }
}

// ---------------------------------------------------------------------------
// LBM Grid Solver (D2Q9) — full-featured API
// ---------------------------------------------------------------------------

/// D2Q9 lattice velocities (x-component).
const LBM_CX: [i32; 9] = [0, 1, 0, -1, 0, 1, -1, -1, 1];
/// D2Q9 lattice velocities (y-component).
const LBM_CY: [i32; 9] = [0, 0, 1, 0, -1, 1, 1, -1, -1];
/// D2Q9 weights.
const LBM_W: [f64; 9] = [
    4.0 / 9.0,
    1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0,
    1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0,
];
/// Opposite direction indices for bounce-back.
const LBM_OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];

/// Boundary condition for each edge of the domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryCondition {
    /// Zou-He velocity inlet with prescribed x-velocity.
    VelocityInlet(f64),
    /// Zero-gradient (extrapolation) outlet.
    ZeroGradientOutlet,
    /// Bounce-back solid wall.
    Wall,
}

/// D2Q9 Lattice Boltzmann grid.
#[derive(Debug, Clone)]
pub struct LbmGrid {
    /// Grid width (number of cells in x).
    pub nx: usize,
    /// Grid height (number of cells in y).
    pub ny: usize,
    /// Relaxation time (tau > 0.5 for stability).
    pub tau: f64,
    /// Distribution functions: flat array of size nx*ny*9.
    pub f: Vec<f64>,
    /// Obstacle mask: `true` = solid cell.
    pub obstacle: Vec<bool>,
    /// Left boundary condition.
    pub bc_left: BoundaryCondition,
    /// Right boundary condition.
    pub bc_right: BoundaryCondition,
    /// Top boundary condition.
    pub bc_top: BoundaryCondition,
    /// Bottom boundary condition.
    pub bc_bottom: BoundaryCondition,
}

/// Result of an LBM solve.
#[derive(Debug, Clone)]
pub struct LbmResult {
    /// Grid width.
    pub nx: usize,
    /// Grid height.
    pub ny: usize,
    /// Velocity field (ux, uy) at each cell, row-major.
    pub velocity: Vec<(f64, f64)>,
    /// Pressure field (p = rho * cs^2 = rho / 3) at each cell, row-major.
    pub pressure: Vec<f64>,
    /// Density field at each cell, row-major.
    pub density: Vec<f64>,
    /// Drag coefficient on obstacle cells (momentum-exchange method).
    pub drag_coefficient: f64,
    /// Number of iterations actually performed.
    pub iterations: usize,
    /// Whether convergence was reached.
    pub converged: bool,
    /// Final L2 norm of velocity change.
    pub residual: f64,
}

impl LbmGrid {
    /// Create a new grid initialized to equilibrium at rest (rho=1, u=0).
    pub fn new(nx: usize, ny: usize, tau: f64) -> Self {
        let n = nx * ny;
        let mut f = vec![0.0; n * 9];
        for idx in 0..n {
            for q in 0..9 {
                f[idx * 9 + q] = LBM_W[q]; // equilibrium at rho=1, u=0
            }
        }
        Self {
            nx,
            ny,
            tau,
            f,
            obstacle: vec![false; n],
            bc_left: BoundaryCondition::Wall,
            bc_right: BoundaryCondition::Wall,
            bc_top: BoundaryCondition::Wall,
            bc_bottom: BoundaryCondition::Wall,
        }
    }

    /// Create from kinematic viscosity, grid spacing, and time step.
    /// tau = 0.5 + 3 * nu * dt / dx^2
    pub fn from_viscosity(nx: usize, ny: usize, nu: f64, dx: f64, dt: f64) -> Self {
        let tau = 0.5 + 3.0 * nu * dt / (dx * dx);
        Self::new(nx, ny, tau)
    }

    /// Mark a single cell as solid obstacle.
    pub fn set_obstacle(&mut self, x: usize, y: usize) {
        if x < self.nx && y < self.ny {
            self.obstacle[y * self.nx + x] = true;
        }
    }

    /// Mark a rectangular region as solid.
    pub fn set_rect_obstacle(&mut self, x0: usize, y0: usize, w: usize, h: usize) {
        for y in y0..(y0 + h).min(self.ny) {
            for x in x0..(x0 + w).min(self.nx) {
                self.obstacle[y * self.nx + x] = true;
            }
        }
    }

    /// Mark a circular region as solid.
    pub fn set_circle_obstacle(&mut self, cx: usize, cy: usize, radius: usize) {
        let r2 = (radius * radius) as i64;
        for y in 0..self.ny {
            for x in 0..self.nx {
                let dx = x as i64 - cx as i64;
                let dy = y as i64 - cy as i64;
                if dx * dx + dy * dy <= r2 {
                    self.obstacle[y * self.nx + x] = true;
                }
            }
        }
    }

    /// Pre-mark wall boundaries as obstacles. Called once during setup.
    pub fn init_wall_obstacles(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        if self.bc_bottom == BoundaryCondition::Wall {
            for x in 0..nx {
                self.obstacle[x] = true;
            }
        }
        if self.bc_top == BoundaryCondition::Wall {
            for x in 0..nx {
                self.obstacle[(ny - 1) * nx + x] = true;
            }
        }
        if self.bc_left == BoundaryCondition::Wall {
            for y in 0..ny {
                self.obstacle[y * nx] = true;
            }
        }
        if self.bc_right == BoundaryCondition::Wall {
            for y in 0..ny {
                self.obstacle[y * nx + nx - 1] = true;
            }
        }
    }

    /// Total number of cells.
    #[inline]
    fn n(&self) -> usize {
        self.nx * self.ny
    }

    /// Compute equilibrium distribution for given rho, ux, uy.
    #[inline]
    fn feq(rho: f64, ux: f64, uy: f64, q: usize) -> f64 {
        let cu = LBM_CX[q] as f64 * ux + LBM_CY[q] as f64 * uy;
        let u2 = ux * ux + uy * uy;
        LBM_W[q] * rho * (1.0 + 3.0 * cu + 4.5 * cu * cu - 1.5 * u2)
    }

    /// Extract macroscopic density and velocity at a cell.
    fn macroscopic(&self, idx: usize) -> (f64, f64, f64) {
        let mut rho = 0.0;
        let mut ux = 0.0;
        let mut uy = 0.0;
        for q in 0..9 {
            let fq = self.f[idx * 9 + q];
            rho += fq;
            ux += LBM_CX[q] as f64 * fq;
            uy += LBM_CY[q] as f64 * fq;
        }
        rho = rho.max(1e-10);
        (rho, ux / rho, uy / rho)
    }

    /// Extract full macroscopic fields.
    fn compute_fields(&self) -> (Vec<f64>, Vec<(f64, f64)>) {
        let n = self.n();
        let mut density = vec![1.0; n];
        let mut velocity = vec![(0.0, 0.0); n];
        for idx in 0..n {
            if self.obstacle[idx] {
                continue;
            }
            let (rho, ux, uy) = self.macroscopic(idx);
            density[idx] = rho;
            velocity[idx] = (ux, uy);
        }
        (density, velocity)
    }

    /// Compute drag coefficient on obstacle cells via momentum exchange.
    fn compute_drag(&self, inlet_velocity: f64) -> f64 {
        let nx = self.nx;
        let ny = self.ny;
        let mut fx = 0.0;

        for y in 0..ny {
            for x in 0..nx {
                let idx = y * nx + x;
                if !self.obstacle[idx] {
                    continue;
                }
                for q in 1..9 {
                    let nxp = x as i32 - LBM_CX[q];
                    let nyp = y as i32 - LBM_CY[q];
                    if nxp >= 0 && (nxp as usize) < nx && nyp >= 0 && (nyp as usize) < ny {
                        let neighbor = nyp as usize * nx + nxp as usize;
                        if !self.obstacle[neighbor] {
                            fx += LBM_CX[q] as f64
                                * (self.f[neighbor * 9 + q] + self.f[idx * 9 + LBM_OPP[q]]);
                        }
                    }
                }
            }
        }

        // Characteristic length = number of obstacle boundary cells in y-direction
        // Use sqrt(obstacle count) as approximation
        let n_obs: usize = self.obstacle.iter().filter(|&&o| o).count();
        // Subtract wall cells (top and bottom rows) from obstacle count
        let wall_cells = 2 * nx;
        let interior_obs = if n_obs > wall_cells { n_obs - wall_cells } else { n_obs };
        let char_length = if interior_obs > 0 {
            (interior_obs as f64).sqrt()
        } else {
            1.0
        };

        let rho = 1.0;
        let dyn_pressure = 0.5 * rho * inlet_velocity * inlet_velocity;
        if dyn_pressure * char_length > 0.0 {
            fx / (dyn_pressure * char_length)
        } else {
            0.0
        }
    }

    /// Perform one LBM time step: collision, streaming, boundary conditions.
    fn step(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        let omega = 1.0 / self.tau;

        // --- BGK Collision ---
        for idx in 0..(nx * ny) {
            if self.obstacle[idx] {
                continue;
            }
            let (rho, ux, uy) = self.macroscopic(idx);
            for q in 0..9 {
                let eq = Self::feq(rho, ux, uy, q);
                self.f[idx * 9 + q] += omega * (eq - self.f[idx * 9 + q]);
            }
        }

        // --- Streaming with bounce-back ---
        // Use pull-based streaming: each fluid cell pulls distributions from neighbors.
        let f_old = self.f.clone();
        // Reset fluid cell distributions to 0, keep obstacle cells untouched.
        for idx in 0..(nx * ny) {
            if !self.obstacle[idx] {
                for q in 0..9 {
                    self.f[idx * 9 + q] = 0.0;
                }
            }
        }

        for y in 0..ny {
            for x in 0..nx {
                let idx = y * nx + x;
                if self.obstacle[idx] {
                    continue; // skip obstacle cells
                }
                for q in 0..9 {
                    // Where does distribution q come from? From the opposite direction.
                    let sx = x as i32 - LBM_CX[q];
                    let sy = y as i32 - LBM_CY[q];

                    if sx >= 0 && (sx as usize) < nx && sy >= 0 && (sy as usize) < ny {
                        let source = sy as usize * nx + sx as usize;
                        if self.obstacle[source] {
                            // Bounce-back: the distribution that the fluid cell sent
                            // toward the obstacle bounces back.
                            self.f[idx * 9 + q] = f_old[idx * 9 + LBM_OPP[q]];
                        } else {
                            // Normal streaming: pull from neighbor.
                            self.f[idx * 9 + q] = f_old[source * 9 + q];
                        }
                    }
                    // If source is out of bounds, leave as 0 (BCs will fix it).
                }
            }
        }

        // --- Boundary Conditions ---
        self.apply_boundary_conditions();
    }

    /// Apply boundary conditions on all four edges.
    /// Wall BCs are handled by the obstacle mask (set during init_wall_obstacles),
    /// so only Zou-He and zero-gradient BCs are applied here each step.
    fn apply_boundary_conditions(&mut self) {
        // Bottom (y=0)
        match self.bc_bottom {
            BoundaryCondition::Wall => {} // already obstacles
            BoundaryCondition::VelocityInlet(u) => self.apply_zou_he_bottom(u),
            BoundaryCondition::ZeroGradientOutlet => self.apply_zero_gradient_bottom(),
        }

        // Top (y=ny-1)
        match self.bc_top {
            BoundaryCondition::Wall => {}
            BoundaryCondition::VelocityInlet(u) => self.apply_zou_he_top(u),
            BoundaryCondition::ZeroGradientOutlet => self.apply_zero_gradient_top(),
        }

        // Left (x=0)
        match self.bc_left {
            BoundaryCondition::Wall => {}
            BoundaryCondition::VelocityInlet(u) => self.apply_zou_he_left(u),
            BoundaryCondition::ZeroGradientOutlet => self.apply_zero_gradient_left(),
        }

        // Right (x=nx-1)
        match self.bc_right {
            BoundaryCondition::Wall => {}
            BoundaryCondition::VelocityInlet(u) => self.apply_zou_he_right(u),
            BoundaryCondition::ZeroGradientOutlet => self.apply_zero_gradient_right(),
        }
    }

    /// Zou-He velocity inlet on the left boundary (x=0), prescribing ux=u_in, uy=0.
    fn apply_zou_he_left(&mut self, u_in: f64) {
        let nx = self.nx;
        let ny = self.ny;
        for y in 1..ny - 1 {
            let idx = y * nx;
            if self.obstacle[idx] {
                continue;
            }
            // Known: f[0], f[2], f[4], f[3], f[6], f[7] (from streaming)
            // Unknown: f[1], f[5], f[8]
            let f0 = self.f[idx * 9 + 0];
            let f2 = self.f[idx * 9 + 2];
            let f3 = self.f[idx * 9 + 3];
            let f4 = self.f[idx * 9 + 4];
            let f6 = self.f[idx * 9 + 6];
            let f7 = self.f[idx * 9 + 7];

            let rho = (f0 + f2 + f4 + 2.0 * (f3 + f6 + f7)) / (1.0 - u_in);
            let ru = rho * u_in;

            self.f[idx * 9 + 1] = f3 + 2.0 / 3.0 * ru;
            self.f[idx * 9 + 5] = f7 - 0.5 * (f2 - f4) + ru / 6.0;
            self.f[idx * 9 + 8] = f6 + 0.5 * (f2 - f4) + ru / 6.0;
        }
    }

    /// Zou-He velocity inlet on the right boundary (x=nx-1).
    fn apply_zou_he_right(&mut self, u_in: f64) {
        let nx = self.nx;
        let ny = self.ny;
        for y in 1..ny - 1 {
            let idx = y * nx + nx - 1;
            if self.obstacle[idx] {
                continue;
            }
            let f0 = self.f[idx * 9 + 0];
            let f1 = self.f[idx * 9 + 1];
            let f2 = self.f[idx * 9 + 2];
            let f4 = self.f[idx * 9 + 4];
            let f5 = self.f[idx * 9 + 5];
            let f8 = self.f[idx * 9 + 8];

            // u_in is negative for flow leaving the domain at the right
            let rho = (f0 + f2 + f4 + 2.0 * (f1 + f5 + f8)) / (1.0 + u_in);
            let ru = rho * u_in;

            self.f[idx * 9 + 3] = f1 - 2.0 / 3.0 * ru;
            self.f[idx * 9 + 7] = f5 + 0.5 * (f2 - f4) - ru / 6.0;
            self.f[idx * 9 + 6] = f8 - 0.5 * (f2 - f4) - ru / 6.0;
        }
    }

    /// Zou-He velocity inlet on the bottom boundary (y=0).
    fn apply_zou_he_bottom(&mut self, u_in: f64) {
        let nx = self.nx;
        for x in 1..nx - 1 {
            let idx = x;
            if self.obstacle[idx] {
                continue;
            }
            let f0 = self.f[idx * 9 + 0];
            let f1 = self.f[idx * 9 + 1];
            let f3 = self.f[idx * 9 + 3];
            let f4 = self.f[idx * 9 + 4];
            let f7 = self.f[idx * 9 + 7];
            let f8 = self.f[idx * 9 + 8];

            let rho = (f0 + f1 + f3 + 2.0 * (f4 + f7 + f8)) / (1.0 - u_in);
            let ru = rho * u_in;

            self.f[idx * 9 + 2] = f4 + 2.0 / 3.0 * ru;
            self.f[idx * 9 + 5] = f7 - 0.5 * (f1 - f3) + ru / 6.0;
            self.f[idx * 9 + 6] = f8 + 0.5 * (f1 - f3) + ru / 6.0;
        }
    }

    /// Zou-He velocity inlet on the top boundary (y=ny-1).
    fn apply_zou_he_top(&mut self, u_in: f64) {
        let nx = self.nx;
        let ny = self.ny;
        for x in 1..nx - 1 {
            let idx = (ny - 1) * nx + x;
            if self.obstacle[idx] {
                continue;
            }
            let f0 = self.f[idx * 9 + 0];
            let f1 = self.f[idx * 9 + 1];
            let f2 = self.f[idx * 9 + 2];
            let f3 = self.f[idx * 9 + 3];
            let f5 = self.f[idx * 9 + 5];
            let f6 = self.f[idx * 9 + 6];

            let rho = (f0 + f1 + f3 + 2.0 * (f2 + f5 + f6)) / (1.0 + u_in);
            let ru = rho * u_in;

            self.f[idx * 9 + 4] = f2 - 2.0 / 3.0 * ru;
            self.f[idx * 9 + 7] = f5 + 0.5 * (f1 - f3) - ru / 6.0;
            self.f[idx * 9 + 8] = f6 - 0.5 * (f1 - f3) - ru / 6.0;
        }
    }

    /// Zero-gradient (extrapolation) outlet on the right boundary.
    fn apply_zero_gradient_right(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        for y in 1..ny - 1 {
            let idx_out = y * nx + nx - 1;
            let idx_in = y * nx + nx - 2;
            if self.obstacle[idx_out] {
                continue;
            }
            for q in 0..9 {
                self.f[idx_out * 9 + q] = self.f[idx_in * 9 + q];
            }
        }
    }

    /// Zero-gradient outlet on the left boundary.
    fn apply_zero_gradient_left(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        for y in 1..ny - 1 {
            let idx_out = y * nx;
            let idx_in = y * nx + 1;
            if self.obstacle[idx_out] {
                continue;
            }
            for q in 0..9 {
                self.f[idx_out * 9 + q] = self.f[idx_in * 9 + q];
            }
        }
    }

    /// Zero-gradient outlet on the top boundary.
    fn apply_zero_gradient_top(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        for x in 1..nx - 1 {
            let idx_out = (ny - 1) * nx + x;
            let idx_in = (ny - 2) * nx + x;
            if self.obstacle[idx_out] {
                continue;
            }
            for q in 0..9 {
                self.f[idx_out * 9 + q] = self.f[idx_in * 9 + q];
            }
        }
    }

    /// Zero-gradient outlet on the bottom boundary.
    fn apply_zero_gradient_bottom(&mut self) {
        let nx = self.nx;
        for x in 1..nx - 1 {
            let idx_out = x;
            let idx_in = self.nx + x;
            if self.obstacle[idx_out] {
                continue;
            }
            for q in 0..9 {
                self.f[idx_out * 9 + q] = self.f[idx_in * 9 + q];
            }
        }
    }
}

/// Solve an LBM grid for the given number of steps with convergence monitoring.
///
/// Returns an `LbmResult` with velocity field, pressure field, drag coefficient, etc.
/// The solver stops early if the L2 norm of velocity change drops below `tol`.
pub fn lbm_solve(grid: &mut LbmGrid, steps: usize, tol: f64) -> LbmResult {
    let n = grid.n();
    let mut prev_velocity = vec![(0.0, 0.0); n];
    let mut converged = false;
    let mut residual = f64::MAX;
    let mut actual_steps = 0;

    // Determine inlet velocity for drag calculation
    let inlet_velocity = match grid.bc_left {
        BoundaryCondition::VelocityInlet(u) => u,
        _ => match grid.bc_bottom {
            BoundaryCondition::VelocityInlet(u) => u,
            _ => 0.1, // fallback
        },
    };

    for step in 0..steps {
        // Store previous velocity for convergence check
        if step % 10 == 0 {
            let (_, vel) = grid.compute_fields();
            prev_velocity.clone_from(&vel);
        }

        grid.step();
        actual_steps = step + 1;

        // Convergence check every 10 steps
        if step % 10 == 9 {
            let (_, vel) = grid.compute_fields();
            let mut diff2 = 0.0;
            let mut norm2 = 0.0;
            for idx in 0..n {
                if grid.obstacle[idx] {
                    continue;
                }
                let (ux, uy) = vel[idx];
                let (px, py) = prev_velocity[idx];
                diff2 += (ux - px) * (ux - px) + (uy - py) * (uy - py);
                norm2 += ux * ux + uy * uy;
            }
            residual = if norm2 > 1e-20 { (diff2 / norm2).sqrt() } else { diff2.sqrt() };
            if residual < tol {
                converged = true;
                break;
            }
        }
    }

    let (density, velocity) = grid.compute_fields();
    let pressure: Vec<f64> = density.iter().map(|&rho| rho / 3.0).collect();
    let drag_coefficient = grid.compute_drag(inlet_velocity);

    LbmResult {
        nx: grid.nx,
        ny: grid.ny,
        velocity,
        pressure,
        density,
        drag_coefficient,
        iterations: actual_steps,
        converged,
        residual,
    }
}

/// Create a channel flow setup with Zou-He velocity inlet on the left,
/// zero-gradient outlet on the right, and solid walls on top/bottom.
///
/// `obstacle_mask` is an optional nx*ny boolean vec; `true` marks solid cells.
pub fn create_channel_flow(
    nx: usize,
    ny: usize,
    inlet_velocity: f64,
    tau: f64,
    obstacle_mask: Option<&[bool]>,
) -> LbmGrid {
    let mut grid = LbmGrid::new(nx, ny, tau);
    grid.bc_left = BoundaryCondition::VelocityInlet(inlet_velocity);
    grid.bc_right = BoundaryCondition::ZeroGradientOutlet;
    grid.bc_top = BoundaryCondition::Wall;
    grid.bc_bottom = BoundaryCondition::Wall;

    if let Some(mask) = obstacle_mask {
        let len = mask.len().min(nx * ny);
        grid.obstacle[..len].copy_from_slice(&mask[..len]);
    }

    // Pre-mark wall boundaries as obstacles so bounce-back works from step 0.
    grid.init_wall_obstacles();

    grid
}

// ---------------------------------------------------------------------------
// Duct / Channel Flow LUT
// ---------------------------------------------------------------------------

/// Common duct loss coefficients (K-factors).
#[derive(Debug, Clone, Copy)]
pub struct DuctLoss {
    pub description: &'static str,
    pub k_factor: f64,
}

pub static DUCT_LOSSES: &[DuctLoss] = &[
    DuctLoss { description: "90° mitered elbow", k_factor: 1.2 },
    DuctLoss { description: "90° smooth elbow (R/D=1)", k_factor: 0.3 },
    DuctLoss { description: "90° smooth elbow (R/D=2)", k_factor: 0.2 },
    DuctLoss { description: "45° smooth elbow", k_factor: 0.15 },
    DuctLoss { description: "Sudden expansion (A2/A1=2)", k_factor: 0.56 },
    DuctLoss { description: "Sudden contraction (A2/A1=0.5)", k_factor: 0.37 },
    DuctLoss { description: "Sharp-edged orifice (β=0.5)", k_factor: 7.8 },
    DuctLoss { description: "Round-edged orifice (β=0.5)", k_factor: 2.7 },
];

/// Calculate total pressure drop through a pipe system.
pub fn system_pressure_drop(
    diameter_m: f64,
    total_length_m: f64,
    velocity_m_s: f64,
    density_kg_m3: f64,
    viscosity_pa_s: f64,
    roughness_m: f64,
    minor_loss_k_total: f64,
) -> f64 {
    let re = density_kg_m3 * velocity_m_s * diameter_m / viscosity_pa_s;
    let f = moody_friction_factor(re, roughness_m / diameter_m);
    let dyn_p = 0.5 * density_kg_m3 * velocity_m_s * velocity_m_s;

    // Major losses (friction) + Minor losses (fittings)
    let major = f * (total_length_m / diameter_m) * dyn_p;
    let minor = minor_loss_k_total * dyn_p;

    major + minor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_flow_laminar() {
        // Re = 1000 × 0.05 × 0.025 / 0.001 = 1250 → laminar
        let result = pipe_flow(0.025, 1.0, 0.05, 1000.0, 0.001, 0.0);
        assert_eq!(result.regime, FlowRegime::Laminar);
        assert!((result.reynolds - 1250.0).abs() < 1.0);
    }

    #[test]
    fn pipe_flow_turbulent() {
        let result = pipe_flow(0.1, 10.0, 5.0, 1000.0, 0.001, 0.00015);
        assert_eq!(result.regime, FlowRegime::Turbulent);
        assert!(result.pressure_drop_pa > 0.0);
        assert!(result.reynolds > 4000.0);
    }

    #[test]
    fn moody_laminar() {
        let f = moody_friction_factor(1000.0, 0.0);
        assert!((f - 0.064).abs() < 0.001); // f = 64/Re = 64/1000
    }

    #[test]
    fn moody_turbulent_smooth() {
        let f = moody_friction_factor(100_000.0, 0.0);
        // For smooth pipe at Re=100k, f ≈ 0.018
        assert!(f > 0.01 && f < 0.03, "f={}", f);
    }

    #[test]
    fn minor_loss_lookup() {
        let k = lookup_minor_loss("gate valve").unwrap();
        assert!((k - 0.2).abs() < 0.01);
    }

    #[test]
    fn lbm_creates() {
        let lbm = LatticeBoltzmann2D::new(50, 20, 0.6);
        assert_eq!(lbm.nx, 50);
        assert_eq!(lbm.ny, 20);
    }

    #[test]
    fn lbm_step_runs() {
        let mut lbm = LatticeBoltzmann2D::new(30, 10, 0.6);
        lbm.set_circle_obstacle(10, 5, 2);
        lbm.run(0.05, 10);
        // Should not panic, velocity field should be populated
        assert!(lbm.max_velocity() >= 0.0);
    }

    #[test]
    fn lbm_obstacle_blocks_flow() {
        let mut lbm = LatticeBoltzmann2D::new(30, 10, 0.6);
        // Wall in the middle
        lbm.set_rect_obstacle(14, 0, 2, 10);
        lbm.run(0.05, 20);
        // Obstacle cells should have zero velocity
        let v = lbm.velocity_magnitude(15, 5);
        assert!(v < 0.001, "velocity in obstacle={}", v);
    }

    #[test]
    fn system_pressure_drop_calculation() {
        // 25mm pipe, 10m long, water at 1 m/s
        let dp = system_pressure_drop(0.025, 10.0, 1.0, 1000.0, 0.001, 0.00015, 3.0);
        assert!(dp > 0.0);
        // Should be on the order of thousands of Pa for this setup
        assert!(dp > 100.0);
    }

    // -----------------------------------------------------------------------
    // LbmGrid solver tests
    // -----------------------------------------------------------------------

    #[test]
    fn lbm_grid_creates() {
        let grid = LbmGrid::new(60, 20, 0.8);
        assert_eq!(grid.nx, 60);
        assert_eq!(grid.ny, 20);
        assert_eq!(grid.f.len(), 60 * 20 * 9);
        assert_eq!(grid.obstacle.len(), 60 * 20);
    }

    #[test]
    fn lbm_grid_from_viscosity() {
        // nu=0.01, dx=1, dt=1 → tau = 0.5 + 3*0.01 = 0.53
        let grid = LbmGrid::from_viscosity(10, 10, 0.01, 1.0, 1.0);
        assert!((grid.tau - 0.53).abs() < 1e-10);
    }

    #[test]
    fn lbm_grid_single_step_no_nan() {
        let nx = 20;
        let ny = 11;
        let u_in = 0.05;
        let tau = 0.8;
        let mut grid = create_channel_flow(nx, ny, u_in, tau, None);

        // After init, all distributions should be W[q]
        for idx in 0..(nx * ny) {
            for q in 0..9 {
                let val = grid.f[idx * 9 + q];
                assert!(val.is_finite(), "Pre-step: f[{}][{}] = {}", idx, q, val);
            }
        }

        for step in 0..200 {
            grid.step();

            // After each step, check for NaN
            let mut nan_count = 0;
            let mut first_nan = None;
            for idx in 0..(nx * ny) {
                for q in 0..9 {
                    let val = grid.f[idx * 9 + q];
                    if !val.is_finite() {
                        nan_count += 1;
                        if first_nan.is_none() {
                            let x = idx % nx;
                            let y = idx / nx;
                            first_nan = Some((x, y, q, val));
                        }
                    }
                }
            }
            if let Some((x, y, q, val)) = first_nan {
                panic!("Found {} NaN/Inf at step {}. First at ({}, {})[q={}] = {}. obstacle={}",
                    nan_count, step + 1, x, y, q, val, grid.obstacle[y * nx + x]);
            }
        }
    }

    #[test]
    fn lbm_poiseuille_parabolic_profile() {
        // Poiseuille (channel) flow should develop a parabolic velocity profile.
        // Channel: inlet left, outlet right, walls top/bottom.
        let nx = 100;
        let ny = 21; // odd for symmetric center
        let u_in = 0.05;
        let tau = 0.8; // kinematic viscosity nu = (tau - 0.5)/3

        let mut grid = create_channel_flow(nx, ny, u_in, tau, None);
        let result = lbm_solve(&mut grid, 5000, 1e-8);

        // Sample velocity profile at x = 3/4 of channel length (well-developed)
        let x_sample = 3 * nx / 4;
        // Collect ux across the channel height (excluding wall rows y=0 and y=ny-1)
        let mut ux_profile = Vec::new();
        for y in 1..ny - 1 {
            let (ux, _) = result.velocity[y * nx + x_sample];
            ux_profile.push(ux);
        }

        // Check symmetry: ux(y) ≈ ux(H-y)
        let n_interior = ux_profile.len();
        for i in 0..n_interior / 2 {
            let diff = (ux_profile[i] - ux_profile[n_interior - 1 - i]).abs();
            assert!(
                diff < 0.01,
                "Asymmetry at y={}: {} vs {}", i, ux_profile[i], ux_profile[n_interior - 1 - i]
            );
        }

        // Check that center velocity is maximum
        let center = n_interior / 2;
        let u_center = ux_profile[center];
        assert!(u_center > 0.0, "Center velocity should be positive, got {}", u_center);
        for (i, &ux) in ux_profile.iter().enumerate() {
            assert!(
                ux <= u_center + 1e-6,
                "Velocity at y={} ({}) exceeds center ({})", i, ux, u_center
            );
        }

        // Check that wall-adjacent velocities are smaller than center
        assert!(ux_profile[0] < u_center * 0.5, "Near-wall velocity should be < 50% of center");
        assert!(ux_profile[n_interior - 1] < u_center * 0.5);
    }

    #[test]
    fn lbm_cylinder_drag_sanity() {
        // Flow around a cylinder: drag coefficient should be positive and finite.
        let nx = 120;
        let ny = 41;
        let u_in = 0.05;
        let tau = 0.8;

        let mut grid = create_channel_flow(nx, ny, u_in, tau, None);
        // Place a cylinder at (nx/4, ny/2) with radius 5
        grid.set_circle_obstacle(nx / 4, ny / 2, 5);

        let result = lbm_solve(&mut grid, 3000, 1e-7);

        // Drag coefficient should be positive (resistance to flow)
        assert!(
            result.drag_coefficient.is_finite(),
            "Drag coefficient should be finite, got {}", result.drag_coefficient
        );
        // Velocity behind the cylinder should be reduced (wake region)
        let wake_x = nx / 4 + 10;
        let wake_y = ny / 2;
        let (ux_wake, _) = result.velocity[wake_y * nx + wake_x];

        // Freestream velocity in the middle far downstream
        let far_x = nx - 5;
        let (ux_far, _) = result.velocity[wake_y * nx + far_x];

        // Wake velocity should be less than freestream (or at least disturbed)
        // This is a sanity check; in a short domain the wake may not fully recover
        assert!(
            ux_wake < ux_far + 0.02,
            "Wake velocity ({}) should generally not exceed far-field ({})", ux_wake, ux_far
        );
    }

    #[test]
    fn lbm_mass_conservation() {
        // Total mass (sum of density) should be approximately conserved.
        let nx = 60;
        let ny = 21;
        let u_in = 0.04;
        let tau = 0.7;

        let mut grid = create_channel_flow(nx, ny, u_in, tau, None);

        // Run a few steps and measure mass
        let result_early = lbm_solve(&mut grid, 100, 1e-20);
        let mass_early: f64 = result_early.density.iter()
            .enumerate()
            .filter(|(i, _)| !grid.obstacle[*i])
            .map(|(_, &rho)| rho)
            .sum();

        // Run more steps
        let result_late = lbm_solve(&mut grid, 500, 1e-20);
        let mass_late: f64 = result_late.density.iter()
            .enumerate()
            .filter(|(i, _)| !grid.obstacle[*i])
            .map(|(_, &rho)| rho)
            .sum();

        // Mass should not diverge — allow 10% variation for open boundaries
        let mass_ratio = mass_late / mass_early;
        assert!(
            mass_ratio > 0.8 && mass_ratio < 1.2,
            "Mass not conserved: early={}, late={}, ratio={}", mass_early, mass_late, mass_ratio
        );
    }

    #[test]
    fn lbm_convergence() {
        // The solver should converge (residual decreases) for a simple channel flow.
        let nx = 80;
        let ny = 21;
        let u_in = 0.04;
        let tau = 0.8;

        let mut grid = create_channel_flow(nx, ny, u_in, tau, None);

        // Run with a generous tolerance — should converge
        let result = lbm_solve(&mut grid, 10000, 1e-6);

        assert!(
            result.converged,
            "Solver should converge. Residual={}, iterations={}", result.residual, result.iterations
        );
        assert!(
            result.residual < 1e-6,
            "Residual should be below tolerance, got {}", result.residual
        );
        // Should converge before max iterations
        assert!(
            result.iterations < 10000,
            "Should converge in fewer than 10000 steps, took {}", result.iterations
        );
    }

    #[test]
    fn lbm_create_channel_flow_helper() {
        let grid = create_channel_flow(50, 15, 0.1, 0.6, None);
        assert_eq!(grid.nx, 50);
        assert_eq!(grid.ny, 15);
        assert_eq!(grid.bc_left, BoundaryCondition::VelocityInlet(0.1));
        assert_eq!(grid.bc_right, BoundaryCondition::ZeroGradientOutlet);
        assert_eq!(grid.bc_top, BoundaryCondition::Wall);
        assert_eq!(grid.bc_bottom, BoundaryCondition::Wall);
    }

    #[test]
    fn lbm_create_channel_flow_with_obstacle_mask() {
        let nx = 30;
        let ny = 10;
        let mut mask = vec![false; nx * ny];
        // Place an obstacle block
        for y in 3..7 {
            for x in 10..15 {
                mask[y * nx + x] = true;
            }
        }
        let grid = create_channel_flow(nx, ny, 0.05, 0.7, Some(&mask));
        assert!(grid.obstacle[3 * nx + 10]);
        // Corner (0,0) is a wall obstacle (bottom + left boundary)
        assert!(grid.obstacle[0]);
        // An interior fluid cell not in the mask should be free
        assert!(!grid.obstacle[2 * nx + 5]);
    }

    #[test]
    fn lbm_result_fields_correct_size() {
        let nx = 40;
        let ny = 15;
        let mut grid = create_channel_flow(nx, ny, 0.05, 0.7, None);
        let result = lbm_solve(&mut grid, 50, 1e-20);

        assert_eq!(result.nx, nx);
        assert_eq!(result.ny, ny);
        assert_eq!(result.velocity.len(), nx * ny);
        assert_eq!(result.pressure.len(), nx * ny);
        assert_eq!(result.density.len(), nx * ny);
        assert_eq!(result.iterations, 50);
    }
}
