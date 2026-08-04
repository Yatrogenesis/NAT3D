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

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! Viewport camera navigation controller.
//!
//! Provides orbit, pan, zoom, and focus controls similar to Blender/Maya.

use nalgebra::{Point3, Vector3};

/// Camera navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    /// Orbit around target (middle mouse drag or Alt+Left).
    Orbit,
    /// Pan camera (Shift+Middle mouse).
    Pan,
    /// Zoom in/out (scroll wheel or Middle drag).
    Zoom,
    /// Free camera movement — see `fly`/`fly_look`.
    /// NOTE: this `NavigationController` is not yet wired into the app viewport,
    /// which drives `state.camera` directly; the fly helpers exist and are tested
    /// but delivering user-facing fly requires porting them to `state.camera` + input.
    Fly,
}

/// View preset directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPreset {
    /// Front view (+Y forward, -Z up).
    Front,
    /// Back view (-Y forward, -Z up).
    Back,
    /// Right view (+X forward, -Z up).
    Right,
    /// Left view (-X forward, -Z up).
    Left,
    /// Top view (+Z forward, +Y up).
    Top,
    /// Bottom view (-Z forward, -Y up).
    Bottom,
    /// Camera view (from active camera object).
    Camera,
}

/// Camera navigation state.
#[derive(Debug, Clone)]
pub struct NavigationController {
    /// Camera position in world space.
    pub camera_position: Point3<f64>,
    /// Target/pivot point the camera orbits around.
    pub target: Point3<f64>,
    /// Up vector (usually +Y or +Z depending on orientation).
    pub up: Vector3<f64>,
    /// Distance from camera to target.
    pub distance: f64,
    /// Horizontal rotation angle (radians, around up axis).
    pub azimuth: f64,
    /// Vertical rotation angle (radians, elevation).
    pub elevation: f64,
    /// Current navigation mode.
    pub mode: NavigationMode,
    /// Pan speed multiplier.
    pub pan_speed: f64,
    /// Orbit speed multiplier.
    pub orbit_speed: f64,
    /// Zoom speed multiplier.
    pub zoom_speed: f64,
    /// Minimum zoom distance.
    pub min_distance: f64,
    /// Maximum zoom distance.
    pub max_distance: f64,
}

impl Default for NavigationController {
    fn default() -> Self {
        Self {
            camera_position: Point3::new(7.35, -6.93, 4.96), // Blender default
            target: Point3::origin(),
            up: Vector3::new(0.0, 0.0, 1.0), // Z-up
            distance: 10.0,
            azimuth: 0.785,   // ~45 degrees
            elevation: 0.524, // ~30 degrees
            mode: NavigationMode::Orbit,
            pan_speed: 0.01,
            orbit_speed: 0.005,
            zoom_speed: 0.1,
            min_distance: 0.1,
            max_distance: 1000.0,
        }
    }
}

impl NavigationController {
    /// Create new navigation controller.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set camera to look at target from current position.
    pub fn look_at(&mut self, eye: Point3<f64>, target: Point3<f64>, up: Vector3<f64>) {
        self.camera_position = eye;
        self.target = target;
        self.up = up.normalize();
        self.distance = (eye - target).norm();

        // Calculate azimuth and elevation from position
        let dir = (target - eye).normalize();
        self.azimuth = dir.y.atan2(dir.x);
        self.elevation = dir.z.asin();
    }

    /// Orbit camera around target.
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        self.azimuth += delta_x * self.orbit_speed;
        self.elevation += delta_y * self.orbit_speed;

        // Clamp elevation to avoid gimbal lock
        self.elevation = self.elevation.clamp(
            -std::f64::consts::FRAC_PI_2 + 0.01,
            std::f64::consts::FRAC_PI_2 - 0.01,
        );

        self.update_camera_position();
    }

    /// Pan camera parallel to view plane.
    pub fn pan(&mut self, delta_x: f64, delta_y: f64) {
        let forward = (self.target - self.camera_position).normalize();
        let right = forward.cross(&self.up).normalize();
        let adjusted_up = right.cross(&forward).normalize();

        let pan_x = right * (delta_x * self.pan_speed * self.distance);
        let pan_y = adjusted_up * (delta_y * self.pan_speed * self.distance);

        let offset = pan_x + pan_y;
        self.camera_position += offset;
        self.target += offset;
    }

    /// Zoom camera in/out.
    pub fn zoom(&mut self, delta: f64) {
        self.distance *= 1.0 - (delta * self.zoom_speed);
        self.distance = self.distance.clamp(self.min_distance, self.max_distance);
        self.update_camera_position();
    }

    /// Focus camera on a point (set as new target).
    pub fn focus(&mut self, point: Point3<f64>) {
        self.target = point;
        self.update_camera_position();
    }

    /// Focus and frame around a bounding box.
    pub fn frame_bounds(&mut self, min: Point3<f64>, max: Point3<f64>) {
        // Calculate center and size
        let center = Point3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        );

        let size = (max - min).norm();

        // Set target to center and adjust distance to frame object
        self.target = center;
        self.distance = size * 1.5; // 1.5x to add some padding
        self.distance = self.distance.clamp(self.min_distance, self.max_distance);

        self.update_camera_position();
    }

    /// Free-fly translation (Fly mode): move the camera and its pivot through world
    /// space along the current view basis (forward/right/up). Because both the camera
    /// and the target shift by the same offset, the view direction and orbit distance
    /// are preserved, so `azimuth`/`elevation` remain valid when switching back to Orbit.
    ///
    /// `forward_amt` moves along the look direction, `right_amt` strafes, `up_amt` rises.
    pub fn fly(&mut self, forward_amt: f64, right_amt: f64, up_amt: f64) {
        let forward = self.forward();
        let right = self.right();
        let up = self.actual_up();
        let offset = forward * forward_amt + right * right_amt + up * up_amt;
        self.camera_position += offset;
        self.target += offset;
    }

    /// Free-fly look (Fly mode): rotate the view direction around the *fixed* camera
    /// position, moving the pivot target. Unlike `orbit` (which moves the camera around
    /// the target), the camera stays anchored — this is first-person mouse-look.
    pub fn fly_look(&mut self, delta_x: f64, delta_y: f64) {
        self.azimuth += delta_x * self.orbit_speed;
        self.elevation += delta_y * self.orbit_speed;

        // Clamp elevation to avoid gimbal lock (same limits as orbit).
        self.elevation = self.elevation.clamp(
            -std::f64::consts::FRAC_PI_2 + 0.01,
            std::f64::consts::FRAC_PI_2 - 0.01,
        );

        // camera = target + dir * distance  (see `update_camera_position`), so with the
        // camera held fixed the new target is camera - dir * distance.
        let dir = Vector3::new(
            self.elevation.cos() * self.azimuth.cos(),
            self.elevation.cos() * self.azimuth.sin(),
            self.elevation.sin(),
        );
        self.target = self.camera_position - dir * self.distance;
    }

    /// Apply view preset.
    pub fn set_view_preset(&mut self, preset: ViewPreset) {
        match preset {
            ViewPreset::Front => {
                self.azimuth = 0.0;
                self.elevation = 0.0;
                self.up = Vector3::new(0.0, 0.0, 1.0);
            }
            ViewPreset::Back => {
                self.azimuth = std::f64::consts::PI;
                self.elevation = 0.0;
                self.up = Vector3::new(0.0, 0.0, 1.0);
            }
            ViewPreset::Right => {
                self.azimuth = std::f64::consts::FRAC_PI_2;
                self.elevation = 0.0;
                self.up = Vector3::new(0.0, 0.0, 1.0);
            }
            ViewPreset::Left => {
                self.azimuth = -std::f64::consts::FRAC_PI_2;
                self.elevation = 0.0;
                self.up = Vector3::new(0.0, 0.0, 1.0);
            }
            ViewPreset::Top => {
                self.azimuth = 0.0;
                self.elevation = std::f64::consts::FRAC_PI_2 - 0.01; // Avoid gimbal lock
                self.up = Vector3::new(0.0, 1.0, 0.0);
            }
            ViewPreset::Bottom => {
                self.azimuth = 0.0;
                self.elevation = -std::f64::consts::FRAC_PI_2 + 0.01;
                self.up = Vector3::new(0.0, -1.0, 0.0);
            }
            ViewPreset::Camera => {
                // TODO: Set from active camera object
                tracing::warn!("Camera view preset not yet implemented");
            }
        }

        self.update_camera_position();
    }

    /// Update camera position from spherical coordinates.
    fn update_camera_position(&mut self) {
        let x = self.distance * self.elevation.cos() * self.azimuth.cos();
        let y = self.distance * self.elevation.cos() * self.azimuth.sin();
        let z = self.distance * self.elevation.sin();

        self.camera_position = self.target + Vector3::new(x, y, z);
    }

    /// Get view matrix for rendering.
    pub fn view_matrix(&self) -> nalgebra::Matrix4<f64> {
        nalgebra::Matrix4::look_at_rh(&self.camera_position, &self.target, &self.up)
    }

    /// Get forward direction vector.
    pub fn forward(&self) -> Vector3<f64> {
        (self.target - self.camera_position).normalize()
    }

    /// Get right direction vector.
    pub fn right(&self) -> Vector3<f64> {
        self.forward().cross(&self.up).normalize()
    }

    /// Get actual up direction vector (orthogonalized).
    pub fn actual_up(&self) -> Vector3<f64> {
        self.right().cross(&self.forward()).normalize()
    }

    /// Reset to default view.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orbit() {
        let mut nav = NavigationController::new();
        let initial_azimuth = nav.azimuth;

        nav.orbit(100.0, 0.0); // Orbit horizontally
        assert!(nav.azimuth > initial_azimuth);

        // Elevation should be clamped
        nav.orbit(0.0, 10000.0); // Try to orbit way up
        assert!(nav.elevation < std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn test_zoom() {
        let mut nav = NavigationController::new();
        let initial_distance = nav.distance;

        nav.zoom(1.0); // Zoom in
        assert!(nav.distance < initial_distance);

        nav.zoom(-10.0); // Zoom out a lot
        assert!(nav.distance >= nav.min_distance);
        assert!(nav.distance <= nav.max_distance);
    }

    #[test]
    fn test_frame_bounds() {
        let mut nav = NavigationController::new();
        let min = Point3::new(-1.0, -1.0, -1.0);
        let max = Point3::new(1.0, 1.0, 1.0);

        nav.frame_bounds(min, max);

        // Target should be at center
        assert!((nav.target.x - 0.0).abs() < 0.001);
        assert!((nav.target.y - 0.0).abs() < 0.001);
        assert!((nav.target.z - 0.0).abs() < 0.001);

        // Distance should be proportional to object size
        let size = (max - min).norm();
        assert!((nav.distance - size * 1.5).abs() < 0.1);
    }

    #[test]
    fn test_view_presets() {
        let mut nav = NavigationController::new();

        // Front view
        nav.set_view_preset(ViewPreset::Front);
        assert!((nav.azimuth - 0.0).abs() < 0.001);
        assert!((nav.elevation - 0.0).abs() < 0.001);

        // Top view
        nav.set_view_preset(ViewPreset::Top);
        assert!((nav.elevation - (std::f64::consts::FRAC_PI_2 - 0.01)).abs() < 0.001);
    }

    #[test]
    fn test_fly_preserves_view_direction() {
        let mut nav = NavigationController::new();
        let fwd0 = nav.forward();
        let rel0 = nav.target - nav.camera_position; // camera→target vector
        let cam0 = nav.camera_position;

        nav.fly(2.0, 1.0, 0.5);

        // Both camera and target shift by the same offset → relative geometry preserved.
        let rel1 = nav.target - nav.camera_position;
        assert!((rel0 - rel1).norm() < 1e-9, "fly must preserve camera→target vector");
        assert!((fwd0 - nav.forward()).norm() < 1e-9, "fly must preserve look direction");
        // The camera actually moved.
        assert!((nav.camera_position - cam0).norm() > 1e-6, "fly must move the camera");
    }

    #[test]
    fn test_fly_look_anchors_camera_and_keeps_distance() {
        let mut nav = NavigationController::new();
        let cam0 = nav.camera_position;
        let dist = nav.distance;

        nav.fly_look(50.0, -20.0);

        // Camera is the anchor in Fly look — it must not move.
        assert!((nav.camera_position - cam0).norm() < 1e-9, "fly_look must keep camera fixed");
        // Target is re-anchored at `distance` along the new view direction.
        assert!(((nav.camera_position - nav.target).norm() - dist).abs() < 1e-9);
        // Elevation stays inside the gimbal-lock clamp.
        assert!(nav.elevation < std::f64::consts::FRAC_PI_2);
        assert!(nav.elevation > -std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn test_direction_vectors() {
        let mut nav = NavigationController::new();

        // Front view should give predictable directions
        nav.set_view_preset(ViewPreset::Front);

        let forward = nav.forward();
        let right = nav.right();
        let up = nav.actual_up();

        // Should be orthonormal
        assert!((forward.norm() - 1.0).abs() < 0.001);
        assert!((right.norm() - 1.0).abs() < 0.001);
        assert!((up.norm() - 1.0).abs() < 0.001);

        // Should be perpendicular
        assert!(forward.dot(&right).abs() < 0.001);
        assert!(forward.dot(&up).abs() < 0.001);
        assert!(right.dot(&up).abs() < 0.001);
    }
}
