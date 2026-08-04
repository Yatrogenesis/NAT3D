/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 *
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 *
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 *
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 *
 * DOI: [PENDING]
 */

//! Full Navier-Stokes solver for accurate fluid simulation.
//!
//! Implements the incompressible Navier-Stokes equations:
//! ∂u/∂t + (u·∇)u = -∇p/ρ + ν∇²u + f
//! ∇·u = 0
//!
//! Uses a stable semi-Lagrangian advection scheme with pressure projection.

use nalgebra::Vector3;

/// Grid-based fluid simulation parameters.
#[derive(Debug, Clone)]
pub struct FluidParams {
    /// Grid resolution in X.
    pub nx: usize,
    /// Grid resolution in Y.
    pub ny: usize,
    /// Grid resolution in Z.
    pub nz: usize,
    /// Cell size.
    pub dx: f64,
    /// Kinematic viscosity.
    pub viscosity: f64,
    /// Fluid density.
    pub density: f64,
    /// Time step.
    pub dt: f64,
    /// Number of pressure solver iterations.
    pub pressure_iterations: usize,
    /// Gravity vector.
    pub gravity: Vector3<f64>,
}

impl Default for FluidParams {
    fn default() -> Self {
        Self {
            nx: 64,
            ny: 64,
            nz: 64,
            dx: 0.1,
            viscosity: 0.001,
            density: 1000.0,
            dt: 0.016,
            pressure_iterations: 50,
            gravity: Vector3::new(0.0, -9.81, 0.0),
        }
    }
}

/// MAC (Marker-and-Cell) grid for staggered velocity storage.
#[derive(Debug, Clone)]
pub struct MacGrid {
    /// Velocity component in X direction (stored on cell faces).
    pub u: Vec<f64>,
    /// Velocity component in Y direction (stored on cell faces).
    pub v: Vec<f64>,
    /// Velocity component in Z direction (stored on cell faces).
    pub w: Vec<f64>,
    /// Pressure (stored at cell centers).
    pub p: Vec<f64>,
    /// Divergence (stored at cell centers).
    pub div: Vec<f64>,
    /// Cell type marker (0=air, 1=fluid, 2=solid).
    pub cell_type: Vec<u8>,
    /// Grid dimension in X.
    pub nx: usize,
    /// Grid dimension in Y.
    pub ny: usize,
    /// Grid dimension in Z.
    pub nz: usize,
}

impl MacGrid {
    /// Create a new MAC grid.
    pub fn new(nx: usize, ny: usize, nz: usize) -> Self {
        let cell_count = nx * ny * nz;
        let u_count = (nx + 1) * ny * nz;
        let v_count = nx * (ny + 1) * nz;
        let w_count = nx * ny * (nz + 1);

        Self {
            u: vec![0.0; u_count],
            v: vec![0.0; v_count],
            w: vec![0.0; w_count],
            p: vec![0.0; cell_count],
            div: vec![0.0; cell_count],
            cell_type: vec![0; cell_count],
            nx,
            ny,
            nz,
        }
    }

    /// Get cell index from coordinates.
    #[inline]
    pub fn cell_idx(&self, i: usize, j: usize, k: usize) -> usize {
        i + j * self.nx + k * self.nx * self.ny
    }

    /// Get U velocity index.
    #[inline]
    pub fn u_idx(&self, i: usize, j: usize, k: usize) -> usize {
        i + j * (self.nx + 1) + k * (self.nx + 1) * self.ny
    }

    /// Get V velocity index.
    #[inline]
    pub fn v_idx(&self, i: usize, j: usize, k: usize) -> usize {
        i + j * self.nx + k * self.nx * (self.ny + 1)
    }

    /// Get W velocity index.
    #[inline]
    pub fn w_idx(&self, i: usize, j: usize, k: usize) -> usize {
        i + j * self.nx + k * self.nx * self.ny
    }

    /// Sample velocity at arbitrary position using trilinear interpolation.
    pub fn sample_velocity(&self, pos: Vector3<f64>, dx: f64) -> Vector3<f64> {
        let u = self.sample_u(pos, dx);
        let v = self.sample_v(pos, dx);
        let w = self.sample_w(pos, dx);
        Vector3::new(u, v, w)
    }

    /// Sample U component with interpolation.
    fn sample_u(&self, pos: Vector3<f64>, dx: f64) -> f64 {
        let x = pos.x / dx;
        let y = pos.y / dx - 0.5;
        let z = pos.z / dx - 0.5;
        self.trilerp_u(x, y, z)
    }

    /// Sample V component with interpolation.
    fn sample_v(&self, pos: Vector3<f64>, dx: f64) -> f64 {
        let x = pos.x / dx - 0.5;
        let y = pos.y / dx;
        let z = pos.z / dx - 0.5;
        self.trilerp_v(x, y, z)
    }

    /// Sample W component with interpolation.
    fn sample_w(&self, pos: Vector3<f64>, dx: f64) -> f64 {
        let x = pos.x / dx - 0.5;
        let y = pos.y / dx - 0.5;
        let z = pos.z / dx;
        self.trilerp_w(x, y, z)
    }

    /// Trilinear interpolation for U.
    fn trilerp_u(&self, x: f64, y: f64, z: f64) -> f64 {
        let i0 = x.floor() as isize;
        let j0 = y.floor() as isize;
        let k0 = z.floor() as isize;

        let fx = x - i0 as f64;
        let fy = y - j0 as f64;
        let fz = z - k0 as f64;

        let mut result = 0.0;
        for di in 0..2 {
            for dj in 0..2 {
                for dk in 0..2 {
                    let i = (i0 + di).clamp(0, self.nx as isize) as usize;
                    let j = (j0 + dj).clamp(0, (self.ny - 1) as isize) as usize;
                    let k = (k0 + dk).clamp(0, (self.nz - 1) as isize) as usize;

                    let wx = if di == 0 { 1.0 - fx } else { fx };
                    let wy = if dj == 0 { 1.0 - fy } else { fy };
                    let wz = if dk == 0 { 1.0 - fz } else { fz };

                    result += wx * wy * wz * self.u[self.u_idx(i, j, k)];
                }
            }
        }
        result
    }

    /// Trilinear interpolation for V.
    fn trilerp_v(&self, x: f64, y: f64, z: f64) -> f64 {
        let i0 = x.floor() as isize;
        let j0 = y.floor() as isize;
        let k0 = z.floor() as isize;

        let fx = x - i0 as f64;
        let fy = y - j0 as f64;
        let fz = z - k0 as f64;

        let mut result = 0.0;
        for di in 0..2 {
            for dj in 0..2 {
                for dk in 0..2 {
                    let i = (i0 + di).clamp(0, (self.nx - 1) as isize) as usize;
                    let j = (j0 + dj).clamp(0, self.ny as isize) as usize;
                    let k = (k0 + dk).clamp(0, (self.nz - 1) as isize) as usize;

                    let wx = if di == 0 { 1.0 - fx } else { fx };
                    let wy = if dj == 0 { 1.0 - fy } else { fy };
                    let wz = if dk == 0 { 1.0 - fz } else { fz };

                    result += wx * wy * wz * self.v[self.v_idx(i, j, k)];
                }
            }
        }
        result
    }

    /// Trilinear interpolation for W.
    fn trilerp_w(&self, x: f64, y: f64, z: f64) -> f64 {
        let i0 = x.floor() as isize;
        let j0 = y.floor() as isize;
        let k0 = z.floor() as isize;

        let fx = x - i0 as f64;
        let fy = y - j0 as f64;
        let fz = z - k0 as f64;

        let mut result = 0.0;
        for di in 0..2 {
            for dj in 0..2 {
                for dk in 0..2 {
                    let i = (i0 + di).clamp(0, (self.nx - 1) as isize) as usize;
                    let j = (j0 + dj).clamp(0, (self.ny - 1) as isize) as usize;
                    let k = (k0 + dk).clamp(0, self.nz as isize) as usize;

                    let wx = if di == 0 { 1.0 - fx } else { fx };
                    let wy = if dj == 0 { 1.0 - fy } else { fy };
                    let wz = if dk == 0 { 1.0 - fz } else { fz };

                    result += wx * wy * wz * self.w[self.w_idx(i, j, k)];
                }
            }
        }
        result
    }
}

/// Navier-Stokes fluid solver.
pub struct NavierStokesSolver {
    /// Simulation parameters.
    pub params: FluidParams,
    /// Current grid state.
    pub grid: MacGrid,
    /// Previous grid state for semi-Lagrangian advection.
    prev_grid: MacGrid,
    /// Simulation time.
    pub time: f64,
    /// Frame count.
    pub frame: u64,
}

impl NavierStokesSolver {
    /// Create a new Navier-Stokes solver.
    pub fn new(params: FluidParams) -> Self {
        let grid = MacGrid::new(params.nx, params.ny, params.nz);
        let prev_grid = grid.clone();

        Self {
            params,
            grid,
            prev_grid,
            time: 0.0,
            frame: 0,
        }
    }

    /// Advance simulation by one time step.
    pub fn step(&mut self) {
        // Save current state
        self.prev_grid = self.grid.clone();

        // 1. Apply external forces (gravity)
        self.apply_forces();

        // 2. Advect velocity (semi-Lagrangian)
        self.advect_velocity();

        // 3. Apply viscosity (diffusion)
        if self.params.viscosity > 0.0 {
            self.apply_viscosity();
        }

        // 4. Compute divergence
        self.compute_divergence();

        // 5. Solve pressure (make velocity divergence-free)
        self.solve_pressure();

        // 6. Apply pressure gradient
        self.apply_pressure_gradient();

        // 7. Enforce boundary conditions
        self.enforce_boundaries();

        self.time += self.params.dt;
        self.frame += 1;
    }

    /// Apply external forces (gravity, etc.).
    fn apply_forces(&mut self) {
        let dt = self.params.dt;
        let g = self.params.gravity;

        // Apply gravity to V component
        for v in &mut self.grid.v {
            *v += g.y * dt;
        }

        // Apply gravity to U and W if needed
        for u in &mut self.grid.u {
            *u += g.x * dt;
        }
        for w in &mut self.grid.w {
            *w += g.z * dt;
        }
    }

    /// Advect velocity using semi-Lagrangian method.
    fn advect_velocity(&mut self) {
        let dt = self.params.dt;
        let dx = self.params.dx;
        let nx = self.params.nx;
        let ny = self.params.ny;
        let nz = self.params.nz;

        // Advect U
        let mut new_u = vec![0.0; self.grid.u.len()];
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..=nx {
                    let pos =
                        Vector3::new(i as f64 * dx, (j as f64 + 0.5) * dx, (k as f64 + 0.5) * dx);
                    let vel = self.prev_grid.sample_velocity(pos, dx);
                    let back_pos = pos - vel * dt;
                    new_u[self.grid.u_idx(i, j, k)] = self.prev_grid.sample_u(back_pos, dx);
                }
            }
        }
        self.grid.u = new_u;

        // Advect V
        let mut new_v = vec![0.0; self.grid.v.len()];
        for k in 0..nz {
            for j in 0..=ny {
                for i in 0..nx {
                    let pos =
                        Vector3::new((i as f64 + 0.5) * dx, j as f64 * dx, (k as f64 + 0.5) * dx);
                    let vel = self.prev_grid.sample_velocity(pos, dx);
                    let back_pos = pos - vel * dt;
                    new_v[self.grid.v_idx(i, j, k)] = self.prev_grid.sample_v(back_pos, dx);
                }
            }
        }
        self.grid.v = new_v;

        // Advect W
        let mut new_w = vec![0.0; self.grid.w.len()];
        for k in 0..=nz {
            for j in 0..ny {
                for i in 0..nx {
                    let pos =
                        Vector3::new((i as f64 + 0.5) * dx, (j as f64 + 0.5) * dx, k as f64 * dx);
                    let vel = self.prev_grid.sample_velocity(pos, dx);
                    let back_pos = pos - vel * dt;
                    new_w[self.grid.w_idx(i, j, k)] = self.prev_grid.sample_w(back_pos, dx);
                }
            }
        }
        self.grid.w = new_w;
    }

    /// Apply viscosity using implicit diffusion.
    fn apply_viscosity(&mut self) {
        let nu = self.params.viscosity;
        let dt = self.params.dt;
        let dx = self.params.dx;
        let alpha = nu * dt / (dx * dx);

        // Gauss-Seidel iteration for diffusion
        for _ in 0..20 {
            self.diffuse_field(
                &mut self.grid.u.clone(),
                alpha,
                self.params.nx + 1,
                self.params.ny,
                self.params.nz,
            );
            self.diffuse_field(
                &mut self.grid.v.clone(),
                alpha,
                self.params.nx,
                self.params.ny + 1,
                self.params.nz,
            );
            self.diffuse_field(
                &mut self.grid.w.clone(),
                alpha,
                self.params.nx,
                self.params.ny,
                self.params.nz + 1,
            );
        }
    }

    /// Diffuse a scalar field.
    fn diffuse_field(&mut self, field: &mut [f64], alpha: f64, nx: usize, ny: usize, nz: usize) {
        let a = alpha;
        let c = 1.0 + 6.0 * a;

        for k in 1..nz - 1 {
            for j in 1..ny - 1 {
                for i in 1..nx - 1 {
                    let idx = i + j * nx + k * nx * ny;
                    let neighbors = field[idx - 1]
                        + field[idx + 1]
                        + field[idx - nx]
                        + field[idx + nx]
                        + field[idx - nx * ny]
                        + field[idx + nx * ny];
                    field[idx] = (field[idx] + a * neighbors) / c;
                }
            }
        }
    }

    /// Compute velocity divergence.
    fn compute_divergence(&mut self) {
        let dx = self.params.dx;
        let nx = self.params.nx;
        let ny = self.params.ny;
        let nz = self.params.nz;
        let scale = 1.0 / dx;

        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = self.grid.cell_idx(i, j, k);

                    let du = self.grid.u[self.grid.u_idx(i + 1, j, k)]
                        - self.grid.u[self.grid.u_idx(i, j, k)];
                    let dv = self.grid.v[self.grid.v_idx(i, j + 1, k)]
                        - self.grid.v[self.grid.v_idx(i, j, k)];
                    let dw = self.grid.w[self.grid.w_idx(i, j, k + 1)]
                        - self.grid.w[self.grid.w_idx(i, j, k)];

                    self.grid.div[idx] = -scale * (du + dv + dw);
                }
            }
        }
    }

    /// Solve pressure using Jacobi iteration.
    ///
    /// Solves ∇²p = div with Neumann boundary conditions.
    fn solve_pressure(&mut self) {
        let nx = self.params.nx;
        let ny = self.params.ny;
        let nz = self.params.nz;
        let dx = self.params.dx;
        let dx2 = dx * dx;

        // Initialize pressure to zero
        for p in &mut self.grid.p {
            *p = 0.0;
        }

        // Jacobi iteration for ∇²p = div
        // Discretized: (p[i+1] + p[i-1] + ... - n*p[i]) / dx² = div
        // => p[i] = (neighbor_sum - div * dx²) / n
        let mut p_new = vec![0.0; self.grid.p.len()];

        for _ in 0..self.params.pressure_iterations {
            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        let idx = self.grid.cell_idx(i, j, k);

                        // Count neighbors and sum pressure
                        let mut neighbor_sum = 0.0;
                        let mut neighbor_count = 0.0;

                        if i > 0 {
                            neighbor_sum += self.grid.p[self.grid.cell_idx(i - 1, j, k)];
                            neighbor_count += 1.0;
                        }
                        if i < nx - 1 {
                            neighbor_sum += self.grid.p[self.grid.cell_idx(i + 1, j, k)];
                            neighbor_count += 1.0;
                        }
                        if j > 0 {
                            neighbor_sum += self.grid.p[self.grid.cell_idx(i, j - 1, k)];
                            neighbor_count += 1.0;
                        }
                        if j < ny - 1 {
                            neighbor_sum += self.grid.p[self.grid.cell_idx(i, j + 1, k)];
                            neighbor_count += 1.0;
                        }
                        if k > 0 {
                            neighbor_sum += self.grid.p[self.grid.cell_idx(i, j, k - 1)];
                            neighbor_count += 1.0;
                        }
                        if k < nz - 1 {
                            neighbor_sum += self.grid.p[self.grid.cell_idx(i, j, k + 1)];
                            neighbor_count += 1.0;
                        }

                        if neighbor_count > 0.0 {
                            // Correct Poisson solver: include dx² factor
                            p_new[idx] = (neighbor_sum - self.grid.div[idx] * dx2) / neighbor_count;
                        }
                    }
                }
            }

            std::mem::swap(&mut self.grid.p, &mut p_new);
        }
    }

    /// Apply pressure gradient to make velocity divergence-free.
    fn apply_pressure_gradient(&mut self) {
        let dx = self.params.dx;
        let scale = 1.0 / dx;
        let nx = self.params.nx;
        let ny = self.params.ny;
        let nz = self.params.nz;

        // Update U
        for k in 0..nz {
            for j in 0..ny {
                for i in 1..nx {
                    let idx = self.grid.u_idx(i, j, k);
                    let p_right = self.grid.p[self.grid.cell_idx(i, j, k)];
                    let p_left = self.grid.p[self.grid.cell_idx(i - 1, j, k)];
                    self.grid.u[idx] -= scale * (p_right - p_left);
                }
            }
        }

        // Update V
        for k in 0..nz {
            for j in 1..ny {
                for i in 0..nx {
                    let idx = self.grid.v_idx(i, j, k);
                    let p_top = self.grid.p[self.grid.cell_idx(i, j, k)];
                    let p_bottom = self.grid.p[self.grid.cell_idx(i, j - 1, k)];
                    self.grid.v[idx] -= scale * (p_top - p_bottom);
                }
            }
        }

        // Update W
        for k in 1..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = self.grid.w_idx(i, j, k);
                    let p_front = self.grid.p[self.grid.cell_idx(i, j, k)];
                    let p_back = self.grid.p[self.grid.cell_idx(i, j, k - 1)];
                    self.grid.w[idx] -= scale * (p_front - p_back);
                }
            }
        }
    }

    /// Enforce boundary conditions.
    fn enforce_boundaries(&mut self) {
        let nx = self.params.nx;
        let ny = self.params.ny;
        let nz = self.params.nz;

        // Collect X boundary indices
        let mut u_indices_0 = Vec::new();
        let mut u_indices_n = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                u_indices_0.push(self.grid.u_idx(0, j, k));
                u_indices_n.push(self.grid.u_idx(nx, j, k));
            }
        }
        for idx in u_indices_0 {
            self.grid.u[idx] = 0.0;
        }
        for idx in u_indices_n {
            self.grid.u[idx] = 0.0;
        }

        // Collect Y boundary indices
        let mut v_indices_0 = Vec::new();
        let mut v_indices_n = Vec::new();
        for k in 0..nz {
            for i in 0..nx {
                v_indices_0.push(self.grid.v_idx(i, 0, k));
                v_indices_n.push(self.grid.v_idx(i, ny, k));
            }
        }
        for idx in v_indices_0 {
            self.grid.v[idx] = 0.0;
        }
        for idx in v_indices_n {
            self.grid.v[idx] = 0.0;
        }

        // Collect Z boundary indices
        let mut w_indices_0 = Vec::new();
        let mut w_indices_n = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                w_indices_0.push(self.grid.w_idx(i, j, 0));
                w_indices_n.push(self.grid.w_idx(i, j, nz));
            }
        }
        for idx in w_indices_0 {
            self.grid.w[idx] = 0.0;
        }
        for idx in w_indices_n {
            self.grid.w[idx] = 0.0;
        }
    }

    /// Add velocity impulse at a position.
    pub fn add_impulse(&mut self, pos: Vector3<f64>, velocity: Vector3<f64>, radius: f64) {
        let dx = self.params.dx;
        let nx = self.params.nx;
        let ny = self.params.ny;
        let nz = self.params.nz;

        let ci = (pos.x / dx) as isize;
        let cj = (pos.y / dx) as isize;
        let ck = (pos.z / dx) as isize;
        let r = (radius / dx) as isize + 1;

        // Collect updates first
        let mut u_updates: Vec<(usize, f64)> = Vec::new();
        let mut v_updates: Vec<(usize, f64)> = Vec::new();
        let mut w_updates: Vec<(usize, f64)> = Vec::new();

        for k in (ck - r).max(0)..(ck + r).min(nz as isize) {
            for j in (cj - r).max(0)..(cj + r).min(ny as isize) {
                for i in (ci - r).max(0)..(ci + r).min(nx as isize) {
                    let cell_pos = Vector3::new(
                        (i as f64 + 0.5) * dx,
                        (j as f64 + 0.5) * dx,
                        (k as f64 + 0.5) * dx,
                    );
                    let dist = (cell_pos - pos).magnitude();
                    if dist < radius {
                        let factor = 1.0 - dist / radius;
                        let i = i as usize;
                        let j = j as usize;
                        let k = k as usize;

                        if i < nx {
                            u_updates.push((self.grid.u_idx(i, j, k), velocity.x * factor));
                            u_updates.push((self.grid.u_idx(i + 1, j, k), velocity.x * factor));
                        }
                        if j < ny {
                            v_updates.push((self.grid.v_idx(i, j, k), velocity.y * factor));
                            v_updates.push((self.grid.v_idx(i, j + 1, k), velocity.y * factor));
                        }
                        if k < nz {
                            w_updates.push((self.grid.w_idx(i, j, k), velocity.z * factor));
                            w_updates.push((self.grid.w_idx(i, j, k + 1), velocity.z * factor));
                        }
                    }
                }
            }
        }

        // Apply updates
        for (idx, val) in u_updates {
            self.grid.u[idx] += val;
        }
        for (idx, val) in v_updates {
            self.grid.v[idx] += val;
        }
        for (idx, val) in w_updates {
            self.grid.w[idx] += val;
        }
    }

    /// Get velocity at a point.
    pub fn velocity_at(&self, pos: Vector3<f64>) -> Vector3<f64> {
        self.grid.sample_velocity(pos, self.params.dx)
    }

    /// Get maximum velocity magnitude.
    pub fn max_velocity(&self) -> f64 {
        let max_u = self.grid.u.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        let max_v = self.grid.v.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        let max_w = self.grid.w.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        (max_u * max_u + max_v * max_v + max_w * max_w).sqrt()
    }

    /// Compute total kinetic energy.
    pub fn kinetic_energy(&self) -> f64 {
        let mut energy = 0.0;
        let dx = self.params.dx;
        let cell_volume = dx * dx * dx;
        let rho = self.params.density;

        for k in 0..self.params.nz {
            for j in 0..self.params.ny {
                for i in 0..self.params.nx {
                    let u = 0.5
                        * (self.grid.u[self.grid.u_idx(i, j, k)]
                            + self.grid.u[self.grid.u_idx(i + 1, j, k)]);
                    let v = 0.5
                        * (self.grid.v[self.grid.v_idx(i, j, k)]
                            + self.grid.v[self.grid.v_idx(i, j + 1, k)]);
                    let w = 0.5
                        * (self.grid.w[self.grid.w_idx(i, j, k)]
                            + self.grid.w[self.grid.w_idx(i, j, k + 1)]);

                    let v_sq = u * u + v * v + w * w;
                    energy += 0.5 * rho * cell_volume * v_sq;
                }
            }
        }

        energy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_creation() {
        let params = FluidParams::default();
        let solver = NavierStokesSolver::new(params);
        assert_eq!(solver.time, 0.0);
        assert_eq!(solver.frame, 0);
    }

    #[test]
    fn test_impulse_and_step() {
        let params = FluidParams {
            nx: 16,
            ny: 16,
            nz: 16,
            ..Default::default()
        };
        let mut solver = NavierStokesSolver::new(params);

        // Add impulse
        solver.add_impulse(
            Vector3::new(0.8, 0.8, 0.8),
            Vector3::new(1.0, 0.0, 0.0),
            0.2,
        );

        // Should have non-zero velocity
        assert!(solver.max_velocity() > 0.0);

        // Step simulation
        solver.step();

        // Time should advance
        assert!(solver.time > 0.0);
        assert_eq!(solver.frame, 1);
    }

    #[test]
    fn test_simulation_stability() {
        // Test that the solver runs without NaN/inf values
        let params = FluidParams {
            nx: 8,
            ny: 8,
            nz: 8,
            dx: 0.1,
            dt: 0.001,
            viscosity: 0.01, // Add viscosity for stability
            gravity: Vector3::zeros(),
            pressure_iterations: 50,
            density: 1000.0,
        };
        let mut solver = NavierStokesSolver::new(params);

        // Apply a small impulse
        solver.add_impulse(
            Vector3::new(0.4, 0.4, 0.4),
            Vector3::new(0.1, 0.0, 0.0),
            0.1,
        );

        let initial_energy = solver.kinetic_energy();
        assert!(initial_energy > 0.0, "Initial energy should be non-zero");
        assert!(
            initial_energy.is_finite(),
            "Initial energy should be finite"
        );

        // Run several steps
        for _ in 0..3 {
            solver.step();
        }

        let final_energy = solver.kinetic_energy();

        // Verify simulation remains stable (no NaN/inf)
        assert!(final_energy.is_finite(), "Final energy should be finite");
        assert!(
            solver.max_velocity().is_finite(),
            "Max velocity should be finite"
        );
    }
}
