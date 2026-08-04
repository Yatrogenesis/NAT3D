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

//! Camera controls for 3D viewport.
//!
//! Implements orbit, pan, zoom, and fly camera modes.

use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};

/// Camera projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    /// Perspective projection.
    Perspective,
    /// Orthographic projection.
    Orthographic,
}

/// Camera navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    /// Orbit around target point.
    Orbit,
    /// Pan camera.
    Pan,
    /// Zoom (dolly).
    Zoom,
    /// First-person fly mode.
    Fly,
    /// Walk mode with gravity.
    Walk,
}

/// Viewport camera state.
#[derive(Debug, Clone)]
pub struct ViewportCamera {
    /// Camera position.
    pub position: Point3<f64>,
    /// Look-at target (for orbit mode).
    pub target: Point3<f64>,
    /// Camera up vector.
    pub up: Vector3<f64>,
    /// Field of view (radians, for perspective).
    pub fov: f64,
    /// Near clipping plane.
    pub near: f64,
    /// Far clipping plane.
    pub far: f64,
    /// Orthographic scale (for orthographic mode).
    pub ortho_scale: f64,
    /// Projection mode.
    pub projection: ProjectionMode,
    /// Current navigation mode.
    pub navigation: NavigationMode,
    /// Aspect ratio.
    pub aspect: f64,
    /// Orbit sensitivity.
    pub orbit_sensitivity: f64,
    /// Pan sensitivity.
    pub pan_sensitivity: f64,
    /// Zoom sensitivity.
    pub zoom_sensitivity: f64,
    /// Fly speed.
    pub fly_speed: f64,
    /// Enable smooth interpolation.
    pub smooth: bool,
    /// Smoothing factor (0-1).
    pub smooth_factor: f64,
}

impl ViewportCamera {
    /// Create a new camera.
    pub fn new() -> Self {
        Self {
            position: Point3::new(5.0, 5.0, 5.0),
            target: Point3::origin(),
            up: Vector3::new(0.0, 1.0, 0.0),
            fov: std::f64::consts::FRAC_PI_4,
            near: 0.1,
            far: 1000.0,
            ortho_scale: 10.0,
            projection: ProjectionMode::Perspective,
            navigation: NavigationMode::Orbit,
            aspect: 16.0 / 9.0,
            orbit_sensitivity: 0.01,
            pan_sensitivity: 0.01,
            zoom_sensitivity: 0.1,
            fly_speed: 5.0,
            smooth: true,
            smooth_factor: 0.15,
        }
    }

    /// Get view matrix.
    pub fn view_matrix(&self) -> Matrix4<f64> {
        Matrix4::look_at_rh(&self.position, &self.target, &self.up)
    }

    /// Get projection matrix.
    pub fn projection_matrix(&self) -> Matrix4<f64> {
        match self.projection {
            ProjectionMode::Perspective => {
                Matrix4::new_perspective(self.aspect, self.fov, self.near, self.far)
            }
            ProjectionMode::Orthographic => {
                let half_height = self.ortho_scale / 2.0;
                let half_width = half_height * self.aspect;
                Matrix4::new_orthographic(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    self.near,
                    self.far,
                )
            }
        }
    }

    /// Get view-projection matrix.
    pub fn view_projection_matrix(&self) -> Matrix4<f64> {
        self.projection_matrix() * self.view_matrix()
    }

    /// Get camera forward direction.
    pub fn forward(&self) -> Vector3<f64> {
        (self.target - self.position).normalize()
    }

    /// Get camera right direction.
    pub fn right(&self) -> Vector3<f64> {
        self.forward().cross(&self.up).normalize()
    }

    /// Get distance to target.
    pub fn distance(&self) -> f64 {
        (self.position - self.target).magnitude()
    }

    /// Orbit camera around target.
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        let _distance = self.distance();
        let offset = self.position - self.target;

        // Horizontal rotation (around up axis)
        let yaw = UnitQuaternion::from_axis_angle(
            &nalgebra::Unit::new_normalize(self.up),
            -delta_x * self.orbit_sensitivity,
        );

        // Vertical rotation (around right axis)
        let right = self.right();
        let pitch = UnitQuaternion::from_axis_angle(
            &nalgebra::Unit::new_normalize(right),
            -delta_y * self.orbit_sensitivity,
        );

        // Apply rotations
        let new_offset = yaw * pitch * offset;

        // Prevent flipping
        let new_up = (yaw * pitch * self.up).normalize();
        if new_up.dot(&Vector3::new(0.0, 1.0, 0.0)) > 0.1 {
            self.position = self.target + new_offset;
        }
    }

    /// Pan camera.
    pub fn pan(&mut self, delta_x: f64, delta_y: f64) {
        let right = self.right();
        let up = right.cross(&self.forward()).normalize();

        let offset = right * (-delta_x * self.pan_sensitivity * self.distance())
            + up * (delta_y * self.pan_sensitivity * self.distance());

        self.position += offset;
        self.target += offset;
    }

    /// Zoom camera (dolly).
    pub fn zoom(&mut self, delta: f64) {
        match self.projection {
            ProjectionMode::Perspective => {
                let direction = self.forward();
                let distance = self.distance();
                let move_amount = delta * self.zoom_sensitivity * distance;

                // Don't get too close to target
                if distance - move_amount > 0.1 {
                    self.position += direction * move_amount;
                }
            }
            ProjectionMode::Orthographic => {
                self.ortho_scale *= 1.0 - delta * self.zoom_sensitivity;
                self.ortho_scale = self.ortho_scale.clamp(0.01, 1000.0);
            }
        }
    }

    /// Fly camera in a direction.
    pub fn fly(&mut self, forward: f64, right: f64, up: f64, dt: f64) {
        let movement = self.forward() * forward + self.right() * right + self.up * up;

        let delta = movement * self.fly_speed * dt;
        self.position += delta;
        self.target += delta;
    }

    /// Look at a point.
    pub fn look_at(&mut self, target: Point3<f64>) {
        self.target = target;
    }

    /// Frame selection (fit objects in view).
    pub fn frame(&mut self, center: Point3<f64>, radius: f64) {
        self.target = center;

        let distance = radius / (self.fov / 2.0).tan() * 1.5;
        let direction = (self.position - self.target).normalize();
        self.position = self.target + direction * distance;

        if self.projection == ProjectionMode::Orthographic {
            self.ortho_scale = radius * 2.5;
        }
    }

    /// Reset camera to default view.
    pub fn reset(&mut self) {
        self.position = Point3::new(5.0, 5.0, 5.0);
        self.target = Point3::origin();
        self.up = Vector3::new(0.0, 1.0, 0.0);
    }

    /// Set standard view (front, back, left, right, top, bottom).
    pub fn set_standard_view(&mut self, view: StandardView) {
        let distance = self.distance();

        match view {
            StandardView::Front => {
                self.position = self.target + Vector3::new(0.0, 0.0, distance);
            }
            StandardView::Back => {
                self.position = self.target + Vector3::new(0.0, 0.0, -distance);
            }
            StandardView::Left => {
                self.position = self.target + Vector3::new(-distance, 0.0, 0.0);
            }
            StandardView::Right => {
                self.position = self.target + Vector3::new(distance, 0.0, 0.0);
            }
            StandardView::Top => {
                self.position = self.target + Vector3::new(0.0, distance, 0.0);
                self.up = Vector3::new(0.0, 0.0, -1.0);
            }
            StandardView::Bottom => {
                self.position = self.target + Vector3::new(0.0, -distance, 0.0);
                self.up = Vector3::new(0.0, 0.0, 1.0);
            }
            StandardView::Isometric => {
                let d = distance / 3.0_f64.sqrt();
                self.position = self.target + Vector3::new(d, d, d);
                self.up = Vector3::new(0.0, 1.0, 0.0);
            }
        }
    }

    /// Convert screen position to world ray.
    pub fn screen_to_ray(
        &self,
        screen_x: f64,
        screen_y: f64,
        width: f64,
        height: f64,
    ) -> (Point3<f64>, Vector3<f64>) {
        let ndc_x = (2.0 * screen_x / width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / height);

        let inv_proj = self
            .projection_matrix()
            .try_inverse()
            .unwrap_or(Matrix4::identity());
        let inv_view = self
            .view_matrix()
            .try_inverse()
            .unwrap_or(Matrix4::identity());

        let ray_clip = nalgebra::Vector4::new(ndc_x, ndc_y, -1.0, 1.0);
        let ray_eye = inv_proj * ray_clip;
        let ray_eye = nalgebra::Vector4::new(ray_eye.x, ray_eye.y, -1.0, 0.0);
        let ray_world = (inv_view * ray_eye).xyz().normalize();

        (self.position, ray_world)
    }
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard orthographic views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardView {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Isometric,
}

/// Smooth camera for interpolated movement.
#[derive(Debug, Clone)]
pub struct SmoothCamera {
    /// Current state.
    pub current: ViewportCamera,
    /// Target state.
    pub target: ViewportCamera,
    /// Interpolation speed.
    pub speed: f64,
}

impl SmoothCamera {
    /// Create a new smooth camera.
    pub fn new(camera: ViewportCamera) -> Self {
        Self {
            current: camera.clone(),
            target: camera,
            speed: 10.0,
        }
    }

    /// Update camera interpolation.
    pub fn update(&mut self, dt: f64) {
        let t = (self.speed * dt).min(1.0);

        self.current.position = lerp_point(self.current.position, self.target.position, t);
        self.current.target = lerp_point(self.current.target, self.target.target, t);
        self.current.ortho_scale = lerp(self.current.ortho_scale, self.target.ortho_scale, t);
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_point(a: Point3<f64>, b: Point3<f64>, t: f64) -> Point3<f64> {
    Point3::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_creation() {
        let camera = ViewportCamera::new();
        assert!(camera.distance() > 0.0);
    }

    #[test]
    fn test_view_matrix() {
        let camera = ViewportCamera::new();
        let view = camera.view_matrix();
        // View matrix should be invertible
        assert!(view.try_inverse().is_some());
    }

    #[test]
    fn test_orbit() {
        let mut camera = ViewportCamera::new();
        let initial_pos = camera.position;

        camera.orbit(0.1, 0.0);

        assert!((camera.position - initial_pos).magnitude() > 0.0);
        // Distance to target should remain constant
        assert!((camera.distance() - (initial_pos - camera.target).magnitude()).abs() < 0.01);
    }

    #[test]
    fn test_zoom() {
        let mut camera = ViewportCamera::new();
        let initial_distance = camera.distance();

        camera.zoom(1.0);

        assert!(camera.distance() < initial_distance);
    }

    #[test]
    fn test_pan() {
        let mut camera = ViewportCamera::new();
        let initial_target = camera.target;

        camera.pan(1.0, 0.0);

        assert!((camera.target - initial_target).magnitude() > 0.0);
    }
}
