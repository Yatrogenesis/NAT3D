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

//! 3D Viewport rendering with wgpu integration.

use std::collections::HashMap;

/// Viewport render mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    Wireframe,
    #[default]
    Solid,
    Textured,
    Rendered,
}

/// Viewport camera projection type.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionType {
    #[default]
    Perspective,
    Orthographic,
}

/// Viewport overlay settings.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OverlaySettings {
    pub show_grid: bool,
    pub show_axes: bool,
    pub show_normals: bool,
    pub show_wireframe_overlay: bool,
    pub show_bounds: bool,
    pub show_stats: bool,
    pub grid_size: f32,
    pub grid_subdivisions: i32,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            show_grid: true,
            show_axes: true,
            show_normals: false,
            show_wireframe_overlay: false,
            show_bounds: false,
            show_stats: true,
            grid_size: 10.0,
            grid_subdivisions: 10,
        }
    }
}

/// Viewport camera for 3D navigation.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ViewportCamera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub projection: ProjectionType,
    pub ortho_scale: f32,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self {
            position: [5.0, 5.0, 5.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov: 45.0,
            near: 0.1,
            far: 1000.0,
            projection: ProjectionType::Perspective,
            ortho_scale: 5.0,
        }
    }
}

#[allow(dead_code)]
impl ViewportCamera {
    pub fn from_state(state: &crate::state::CameraState) -> Self {
        Self {
            position: state.position,
            target: state.target,
            up: [0.0, 1.0, 0.0],
            fov: state.fov,
            near: 0.1,
            far: 1000.0,
            projection: ProjectionType::Perspective,
            ortho_scale: 1.0,
        }
    }
    /// Create view matrix.
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        let f = [
            self.target[0] - self.position[0],
            self.target[1] - self.position[1],
            self.target[2] - self.position[2],
        ];
        let f_len = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
        let f = [f[0] / f_len, f[1] / f_len, f[2] / f_len];

        let s = [
            f[1] * self.up[2] - f[2] * self.up[1],
            f[2] * self.up[0] - f[0] * self.up[2],
            f[0] * self.up[1] - f[1] * self.up[0],
        ];
        let s_len = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        let s = [s[0] / s_len, s[1] / s_len, s[2] / s_len];

        let u = [
            s[1] * f[2] - s[2] * f[1],
            s[2] * f[0] - s[0] * f[2],
            s[0] * f[1] - s[1] * f[0],
        ];

        [
            [s[0], u[0], -f[0], 0.0],
            [s[1], u[1], -f[1], 0.0],
            [s[2], u[2], -f[2], 0.0],
            [
                -s[0] * self.position[0] - s[1] * self.position[1] - s[2] * self.position[2],
                -u[0] * self.position[0] - u[1] * self.position[1] - u[2] * self.position[2],
                f[0] * self.position[0] + f[1] * self.position[1] + f[2] * self.position[2],
                1.0,
            ],
        ]
    }

    /// Create projection matrix.
    pub fn projection_matrix(&self, aspect: f32) -> [[f32; 4]; 4] {
        match self.projection {
            ProjectionType::Perspective => {
                let f = 1.0 / (self.fov.to_radians() / 2.0).tan();
                let nf = 1.0 / (self.near - self.far);
                [
                    [f / aspect, 0.0, 0.0, 0.0],
                    [0.0, f, 0.0, 0.0],
                    [0.0, 0.0, (self.far + self.near) * nf, -1.0],
                    [0.0, 0.0, 2.0 * self.far * self.near * nf, 0.0],
                ]
            }
            ProjectionType::Orthographic => {
                let s = self.ortho_scale;
                let nf = 1.0 / (self.near - self.far);
                [
                    [1.0 / (s * aspect), 0.0, 0.0, 0.0],
                    [0.0, 1.0 / s, 0.0, 0.0],
                    [0.0, 0.0, 2.0 * nf, 0.0],
                    [0.0, 0.0, (self.far + self.near) * nf, 1.0],
                ]
            }
        }
    }

    /// Orbit around target.
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        let offset = [
            self.position[0] - self.target[0],
            self.position[1] - self.target[1],
            self.position[2] - self.target[2],
        ];

        let radius = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt();
        let mut theta = offset[0].atan2(offset[2]);
        let mut phi = (offset[1] / radius).acos();

        theta += delta_x * 0.01;
        phi = (phi - delta_y * 0.01).clamp(0.1, std::f32::consts::PI - 0.1);

        self.position[0] = self.target[0] + radius * phi.sin() * theta.sin();
        self.position[1] = self.target[1] + radius * phi.cos();
        self.position[2] = self.target[2] + radius * phi.sin() * theta.cos();
    }

    /// Pan camera.
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let forward = [
            self.target[0] - self.position[0],
            self.target[1] - self.position[1],
            self.target[2] - self.position[2],
        ];
        let right = [
            forward[1] * self.up[2] - forward[2] * self.up[1],
            forward[2] * self.up[0] - forward[0] * self.up[2],
            forward[0] * self.up[1] - forward[1] * self.up[0],
        ];
        let right_len = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2]).sqrt();
        let right = [
            right[0] / right_len,
            right[1] / right_len,
            right[2] / right_len,
        ];

        let pan_speed = 0.01;
        let dx = delta_x * pan_speed;
        let dy = delta_y * pan_speed;

        self.position[0] += right[0] * dx + self.up[0] * dy;
        self.position[1] += right[1] * dx + self.up[1] * dy;
        self.position[2] += right[2] * dx + self.up[2] * dy;
        self.target[0] += right[0] * dx + self.up[0] * dy;
        self.target[1] += right[1] * dx + self.up[1] * dy;
        self.target[2] += right[2] * dx + self.up[2] * dy;
    }

    /// Zoom camera.
    pub fn zoom(&mut self, delta: f32) {
        match self.projection {
            ProjectionType::Perspective => {
                let dir = [
                    self.position[0] - self.target[0],
                    self.position[1] - self.target[1],
                    self.position[2] - self.target[2],
                ];
                let dist = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                let new_dist = (dist * (1.0 - delta * 0.1)).max(0.5);
                let factor = new_dist / dist;
                self.position[0] = self.target[0] + dir[0] * factor;
                self.position[1] = self.target[1] + dir[1] * factor;
                self.position[2] = self.target[2] + dir[2] * factor;
            }
            ProjectionType::Orthographic => {
                self.ortho_scale = (self.ortho_scale * (1.0 - delta * 0.1)).max(0.1);
            }
        }
    }

    /// Focus on a point.
    pub fn focus_on(&mut self, point: [f32; 3], distance: f32) {
        let dir = [
            self.position[0] - self.target[0],
            self.position[1] - self.target[1],
            self.position[2] - self.target[2],
        ];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let dir = [dir[0] / len, dir[1] / len, dir[2] / len];

        self.target = point;
        self.position = [
            point[0] + dir[0] * distance,
            point[1] + dir[1] * distance,
            point[2] + dir[2] * distance,
        ];
    }

    /// Set to front view.
    pub fn set_front_view(&mut self) {
        let dist = self.distance_to_target();
        self.position = [self.target[0], self.target[1], self.target[2] + dist];
        self.up = [0.0, 1.0, 0.0];
    }

    /// Set to back view.
    pub fn set_back_view(&mut self) {
        let dist = self.distance_to_target();
        self.position = [self.target[0], self.target[1], self.target[2] - dist];
        self.up = [0.0, 1.0, 0.0];
    }

    /// Set to right view.
    pub fn set_right_view(&mut self) {
        let dist = self.distance_to_target();
        self.position = [self.target[0] + dist, self.target[1], self.target[2]];
        self.up = [0.0, 1.0, 0.0];
    }

    /// Set to left view.
    pub fn set_left_view(&mut self) {
        let dist = self.distance_to_target();
        self.position = [self.target[0] - dist, self.target[1], self.target[2]];
        self.up = [0.0, 1.0, 0.0];
    }

    /// Set to top view.
    pub fn set_top_view(&mut self) {
        let dist = self.distance_to_target();
        self.position = [self.target[0], self.target[1] + dist, self.target[2]];
        self.up = [0.0, 0.0, -1.0];
    }

    /// Set to bottom view.
    pub fn set_bottom_view(&mut self) {
        let dist = self.distance_to_target();
        self.position = [self.target[0], self.target[1] - dist, self.target[2]];
        self.up = [0.0, 0.0, 1.0];
    }

    fn distance_to_target(&self) -> f32 {
        let d = [
            self.position[0] - self.target[0],
            self.position[1] - self.target[1],
            self.position[2] - self.target[2],
        ];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }
}

/// GPU mesh data for rendering.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct GpuMesh {
    pub vertex_count: u32,
    pub index_count: u32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

/// Viewport render statistics.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub triangles: u32,
    pub vertices: u32,
    pub gpu_memory_mb: f32,
}

/// 3D Viewport with wgpu rendering support.
#[allow(dead_code)]
pub struct Viewport3D {
    pub camera: ViewportCamera,
    pub overlay: OverlaySettings,
    pub render_mode: RenderMode,
    pub stats: RenderStats,
    meshes: HashMap<u64, GpuMesh>,
    next_mesh_id: u64,
    pub background_color: [f32; 4],
    pub selection_color: [f32; 4],
    pub grid_color: [f32; 4],
}

#[allow(dead_code)]
impl Viewport3D {
    /// Create a new 3D viewport.
    pub fn new() -> Self {
        Self {
            camera: ViewportCamera::default(),
            overlay: OverlaySettings::default(),
            render_mode: RenderMode::default(),
            stats: RenderStats::default(),
            meshes: HashMap::new(),
            next_mesh_id: 1,
            background_color: [0.1, 0.1, 0.12, 1.0],
            selection_color: [1.0, 0.5, 0.0, 1.0],
            grid_color: [0.3, 0.3, 0.3, 0.5],
        }
    }

    /// Register a mesh for rendering.
    pub fn register_mesh(&mut self, vertex_count: u32, index_count: u32) -> u64 {
        let id = self.next_mesh_id;
        self.next_mesh_id += 1;
        self.meshes.insert(
            id,
            GpuMesh {
                vertex_count,
                index_count,
                bounds_min: [-1.0, -1.0, -1.0],
                bounds_max: [1.0, 1.0, 1.0],
            },
        );
        id
    }

    /// Remove a mesh.
    pub fn remove_mesh(&mut self, id: u64) {
        self.meshes.remove(&id);
    }

    /// Get mesh count.
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Update render stats.
    pub fn update_stats(
        &mut self,
        frame_time: f32,
        draw_calls: u32,
        triangles: u32,
        vertices: u32,
    ) {
        self.stats.frame_time_ms = frame_time;
        self.stats.draw_calls = draw_calls;
        self.stats.triangles = triangles;
        self.stats.vertices = vertices;
    }

    /// Get FPS from frame time.
    pub fn fps(&self) -> f32 {
        if self.stats.frame_time_ms > 0.0 {
            1000.0 / self.stats.frame_time_ms
        } else {
            0.0
        }
    }

    /// Project a 3D point to 2D screen coordinates.
    pub fn project(&self, point: [f32; 3], viewport_size: [f32; 2]) -> Option<[f32; 2]> {
        let view = self.camera.view_matrix();
        let proj = self
            .camera
            .projection_matrix(viewport_size[0] / viewport_size[1]);

        // Transform point
        let view_pos = [
            view[0][0] * point[0] + view[1][0] * point[1] + view[2][0] * point[2] + view[3][0],
            view[0][1] * point[0] + view[1][1] * point[1] + view[2][1] * point[2] + view[3][1],
            view[0][2] * point[0] + view[1][2] * point[1] + view[2][2] * point[2] + view[3][2],
            view[0][3] * point[0] + view[1][3] * point[1] + view[2][3] * point[2] + view[3][3],
        ];

        let clip_pos = [
            proj[0][0] * view_pos[0]
                + proj[1][0] * view_pos[1]
                + proj[2][0] * view_pos[2]
                + proj[3][0] * view_pos[3],
            proj[0][1] * view_pos[0]
                + proj[1][1] * view_pos[1]
                + proj[2][1] * view_pos[2]
                + proj[3][1] * view_pos[3],
            proj[0][2] * view_pos[0]
                + proj[1][2] * view_pos[1]
                + proj[2][2] * view_pos[2]
                + proj[3][2] * view_pos[3],
            proj[0][3] * view_pos[0]
                + proj[1][3] * view_pos[1]
                + proj[2][3] * view_pos[2]
                + proj[3][3] * view_pos[3],
        ];

        if clip_pos[3] <= 0.0 {
            return None;
        }

        let ndc = [clip_pos[0] / clip_pos[3], clip_pos[1] / clip_pos[3]];
        Some([
            (ndc[0] + 1.0) * 0.5 * viewport_size[0],
            (1.0 - ndc[1]) * 0.5 * viewport_size[1],
        ])
    }

    /// Unproject a 2D screen point to a 3D ray.
    pub fn unproject(&self, screen_pos: [f32; 2], viewport_size: [f32; 2]) -> ([f32; 3], [f32; 3]) {
        let _ndc = [
            screen_pos[0] / viewport_size[0] * 2.0 - 1.0,
            1.0 - screen_pos[1] / viewport_size[1] * 2.0,
        ];

        // For now, return camera position and direction toward target
        let dir = [
            self.camera.target[0] - self.camera.position[0],
            self.camera.target[1] - self.camera.position[1],
            self.camera.target[2] - self.camera.position[2],
        ];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();

        (
            self.camera.position,
            [dir[0] / len, dir[1] / len, dir[2] / len],
        )
    }
}

impl Default for Viewport3D {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-viewport layout.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportLayout {
    #[default]
    Single,
    TwoHorizontal,
    TwoVertical,
    Quad,
    ThreeLeft,
    ThreeRight,
}

/// Multi-viewport manager.
#[allow(dead_code)]
pub struct ViewportManager {
    pub layout: ViewportLayout,
    pub viewports: Vec<Viewport3D>,
    pub active_viewport: usize,
}

#[allow(dead_code)]
impl ViewportManager {
    /// Create a new viewport manager.
    pub fn new() -> Self {
        let mut manager = Self {
            layout: ViewportLayout::Single,
            viewports: Vec::new(),
            active_viewport: 0,
        };
        manager.viewports.push(Viewport3D::new());
        manager
    }

    /// Set layout.
    pub fn set_layout(&mut self, layout: ViewportLayout) {
        self.layout = layout;
        let required = match layout {
            ViewportLayout::Single => 1,
            ViewportLayout::TwoHorizontal | ViewportLayout::TwoVertical => 2,
            ViewportLayout::ThreeLeft | ViewportLayout::ThreeRight => 3,
            ViewportLayout::Quad => 4,
        };
        while self.viewports.len() < required {
            let mut vp = Viewport3D::new();
            // Set different default views
            match self.viewports.len() {
                1 => vp.camera.set_front_view(),
                2 => vp.camera.set_right_view(),
                3 => vp.camera.set_top_view(),
                _ => {}
            }
            self.viewports.push(vp);
        }
    }

    /// Get active viewport.
    pub fn active(&self) -> &Viewport3D {
        &self.viewports[self.active_viewport]
    }

    /// Get active viewport mutably.
    pub fn active_mut(&mut self) -> &mut Viewport3D {
        &mut self.viewports[self.active_viewport]
    }
}

impl Default for ViewportManager {
    fn default() -> Self {
        Self::new()
    }
}
