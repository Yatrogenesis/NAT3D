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

//! Transform gizmos for 3D viewport.
//!
//! Visual manipulation tools for move, rotate, and scale operations.

use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};

/// Gizmo operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    /// Translate (move).
    Translate,
    /// Rotate.
    Rotate,
    /// Scale.
    Scale,
}

/// Gizmo coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoSpace {
    /// World coordinates.
    World,
    /// Local object coordinates.
    Local,
}

/// Gizmo axis or component being manipulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    /// No axis selected.
    None,
    /// X axis.
    X,
    /// Y axis.
    Y,
    /// Z axis.
    Z,
    /// XY plane.
    XY,
    /// XZ plane.
    XZ,
    /// YZ plane.
    YZ,
    /// All axes (uniform scale).
    XYZ,
    /// View-aligned.
    View,
}

/// Gizmo visual configuration.
#[derive(Debug, Clone)]
pub struct GizmoConfig {
    /// Gizmo size in screen pixels.
    pub size: f64,
    /// Axis line thickness.
    pub thickness: f64,
    /// Handle size.
    pub handle_size: f64,
    /// Selection threshold.
    pub selection_threshold: f64,
    /// X axis color.
    pub x_color: [f64; 4],
    /// Y axis color.
    pub y_color: [f64; 4],
    /// Z axis color.
    pub z_color: [f64; 4],
    /// Selected axis color.
    pub selected_color: [f64; 4],
    /// Plane color alpha.
    pub plane_alpha: f64,
}

impl Default for GizmoConfig {
    fn default() -> Self {
        Self {
            size: 100.0,
            thickness: 2.0,
            handle_size: 10.0,
            selection_threshold: 15.0,
            x_color: [1.0, 0.2, 0.2, 1.0],
            y_color: [0.2, 1.0, 0.2, 1.0],
            z_color: [0.2, 0.2, 1.0, 1.0],
            selected_color: [1.0, 1.0, 0.2, 1.0],
            plane_alpha: 0.3,
        }
    }
}

/// 3D transform gizmo.
#[derive(Debug, Clone)]
pub struct Gizmo {
    /// Current mode.
    pub mode: GizmoMode,
    /// Coordinate space.
    pub space: GizmoSpace,
    /// Currently hovered/selected axis.
    pub active_axis: GizmoAxis,
    /// Gizmo center position.
    pub position: Point3<f64>,
    /// Gizmo orientation (for local space).
    pub orientation: UnitQuaternion<f64>,
    /// Configuration.
    pub config: GizmoConfig,
    /// Is currently being dragged.
    pub dragging: bool,
    /// Drag start point.
    drag_start: Option<Point3<f64>>,
    /// Initial transform when drag started.
    initial_transform: Option<TransformState>,
}

/// Captured transform state for undo/redo.
#[derive(Debug, Clone)]
pub struct TransformState {
    pub position: Vector3<f64>,
    pub rotation: UnitQuaternion<f64>,
    pub scale: Vector3<f64>,
}

impl Default for TransformState {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Gizmo {
    /// Create a new gizmo.
    pub fn new() -> Self {
        Self {
            mode: GizmoMode::Translate,
            space: GizmoSpace::World,
            active_axis: GizmoAxis::None,
            position: Point3::origin(),
            orientation: UnitQuaternion::identity(),
            config: GizmoConfig::default(),
            dragging: false,
            drag_start: None,
            initial_transform: None,
        }
    }

    /// Set gizmo position.
    pub fn set_position(&mut self, position: Point3<f64>) {
        self.position = position;
    }

    /// Set gizmo orientation.
    pub fn set_orientation(&mut self, orientation: UnitQuaternion<f64>) {
        self.orientation = orientation;
    }

    /// Get axis direction in world space.
    pub fn axis_direction(&self, axis: GizmoAxis) -> Option<Vector3<f64>> {
        let dir = match axis {
            GizmoAxis::X => Vector3::new(1.0, 0.0, 0.0),
            GizmoAxis::Y => Vector3::new(0.0, 1.0, 0.0),
            GizmoAxis::Z => Vector3::new(0.0, 0.0, 1.0),
            _ => return None,
        };

        match self.space {
            GizmoSpace::World => Some(dir),
            GizmoSpace::Local => Some(self.orientation * dir),
        }
    }

    /// Get plane normal in world space.
    pub fn plane_normal(&self, axis: GizmoAxis) -> Option<Vector3<f64>> {
        match axis {
            GizmoAxis::XY => self.axis_direction(GizmoAxis::Z),
            GizmoAxis::XZ => self.axis_direction(GizmoAxis::Y),
            GizmoAxis::YZ => self.axis_direction(GizmoAxis::X),
            _ => None,
        }
    }

    /// Hit test against gizmo.
    pub fn hit_test(
        &self,
        ray_origin: Point3<f64>,
        ray_dir: Vector3<f64>,
        view_matrix: &Matrix4<f64>,
    ) -> GizmoAxis {
        match self.mode {
            GizmoMode::Translate => self.hit_test_translate(ray_origin, ray_dir, view_matrix),
            GizmoMode::Rotate => self.hit_test_rotate(ray_origin, ray_dir, view_matrix),
            GizmoMode::Scale => self.hit_test_scale(ray_origin, ray_dir, view_matrix),
        }
    }

    fn hit_test_translate(
        &self,
        ray_origin: Point3<f64>,
        ray_dir: Vector3<f64>,
        _view_matrix: &Matrix4<f64>,
    ) -> GizmoAxis {
        let threshold = self.config.selection_threshold * 0.01;

        // Test each axis
        for (axis, dir) in [
            (GizmoAxis::X, Vector3::x()),
            (GizmoAxis::Y, Vector3::y()),
            (GizmoAxis::Z, Vector3::z()),
        ] {
            let axis_dir = match self.space {
                GizmoSpace::World => dir,
                GizmoSpace::Local => self.orientation * dir,
            };

            if let Some(dist) = ray_to_line_distance(ray_origin, ray_dir, self.position, axis_dir) {
                if dist < threshold {
                    return axis;
                }
            }
        }

        GizmoAxis::None
    }

    fn hit_test_rotate(
        &self,
        ray_origin: Point3<f64>,
        ray_dir: Vector3<f64>,
        _view_matrix: &Matrix4<f64>,
    ) -> GizmoAxis {
        let radius = 1.0; // Normalized gizmo radius
        let threshold = self.config.selection_threshold * 0.01;

        // Test each rotation ring
        for (axis, normal) in [
            (GizmoAxis::X, Vector3::x()),
            (GizmoAxis::Y, Vector3::y()),
            (GizmoAxis::Z, Vector3::z()),
        ] {
            let plane_normal = match self.space {
                GizmoSpace::World => normal,
                GizmoSpace::Local => self.orientation * normal,
            };

            if let Some(t) =
                ray_plane_intersection(ray_origin, ray_dir, self.position, plane_normal)
            {
                let hit_point = ray_origin + ray_dir * t;
                let dist_to_center = (hit_point - self.position).magnitude();

                if (dist_to_center - radius).abs() < threshold {
                    return axis;
                }
            }
        }

        GizmoAxis::None
    }

    fn hit_test_scale(
        &self,
        ray_origin: Point3<f64>,
        ray_dir: Vector3<f64>,
        view_matrix: &Matrix4<f64>,
    ) -> GizmoAxis {
        // Scale gizmo hit test is similar to translate
        self.hit_test_translate(ray_origin, ray_dir, view_matrix)
    }

    /// Begin drag operation.
    pub fn begin_drag(&mut self, axis: GizmoAxis, ray_origin: Point3<f64>, ray_dir: Vector3<f64>) {
        self.active_axis = axis;
        self.dragging = true;
        self.drag_start = self.compute_drag_point(ray_origin, ray_dir);
        self.initial_transform = Some(TransformState {
            position: self.position.coords,
            rotation: self.orientation,
            scale: Vector3::new(1.0, 1.0, 1.0),
        });
    }

    /// Update drag operation.
    pub fn update_drag(
        &mut self,
        ray_origin: Point3<f64>,
        ray_dir: Vector3<f64>,
    ) -> Option<TransformDelta> {
        if !self.dragging {
            return None;
        }

        let current_point = self.compute_drag_point(ray_origin, ray_dir)?;
        let start_point = self.drag_start?;

        match self.mode {
            GizmoMode::Translate => {
                let delta = current_point - start_point;
                Some(TransformDelta::Translate(delta))
            }
            GizmoMode::Rotate => {
                // Compute rotation angle from drag
                let to_start = (start_point - self.position).normalize();
                let to_current = (current_point - self.position).normalize();
                let axis = to_start.cross(&to_current);

                if axis.magnitude() > 1e-6 {
                    let angle = to_start.dot(&to_current).acos();
                    let rotation = UnitQuaternion::from_axis_angle(
                        &nalgebra::Unit::new_normalize(axis),
                        angle,
                    );
                    Some(TransformDelta::Rotate(rotation))
                } else {
                    None
                }
            }
            GizmoMode::Scale => {
                let start_dist = (start_point - self.position).magnitude();
                let current_dist = (current_point - self.position).magnitude();

                if start_dist > 1e-6 {
                    let scale_factor = current_dist / start_dist;
                    let scale = match self.active_axis {
                        GizmoAxis::X => Vector3::new(scale_factor, 1.0, 1.0),
                        GizmoAxis::Y => Vector3::new(1.0, scale_factor, 1.0),
                        GizmoAxis::Z => Vector3::new(1.0, 1.0, scale_factor),
                        GizmoAxis::XYZ => Vector3::new(scale_factor, scale_factor, scale_factor),
                        _ => Vector3::new(1.0, 1.0, 1.0),
                    };
                    Some(TransformDelta::Scale(scale))
                } else {
                    None
                }
            }
        }
    }

    /// End drag operation.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_start = None;
        self.active_axis = GizmoAxis::None;
    }

    /// Compute drag point on constraint plane/axis.
    fn compute_drag_point(
        &self,
        ray_origin: Point3<f64>,
        ray_dir: Vector3<f64>,
    ) -> Option<Point3<f64>> {
        match self.active_axis {
            GizmoAxis::X | GizmoAxis::Y | GizmoAxis::Z => {
                // Project onto axis
                let axis_dir = self.axis_direction(self.active_axis)?;
                closest_point_on_line_to_ray(self.position, axis_dir, ray_origin, ray_dir)
            }
            GizmoAxis::XY | GizmoAxis::XZ | GizmoAxis::YZ => {
                // Intersect with plane
                let normal = self.plane_normal(self.active_axis)?;
                let t = ray_plane_intersection(ray_origin, ray_dir, self.position, normal)?;
                Some(ray_origin + ray_dir * t)
            }
            _ => None,
        }
    }

    /// Get color for an axis.
    pub fn axis_color(&self, axis: GizmoAxis) -> [f64; 4] {
        if axis == self.active_axis {
            self.config.selected_color
        } else {
            match axis {
                GizmoAxis::X => self.config.x_color,
                GizmoAxis::Y => self.config.y_color,
                GizmoAxis::Z => self.config.z_color,
                _ => [0.5, 0.5, 0.5, 1.0],
            }
        }
    }
}

impl Default for Gizmo {
    fn default() -> Self {
        Self::new()
    }
}

/// Transform delta from gizmo manipulation.
#[derive(Debug, Clone)]
pub enum TransformDelta {
    /// Translation delta.
    Translate(Vector3<f64>),
    /// Rotation delta.
    Rotate(UnitQuaternion<f64>),
    /// Scale delta.
    Scale(Vector3<f64>),
}

// Utility functions

fn ray_to_line_distance(
    ray_origin: Point3<f64>,
    ray_dir: Vector3<f64>,
    line_point: Point3<f64>,
    line_dir: Vector3<f64>,
) -> Option<f64> {
    let w = ray_origin - line_point;
    let a = ray_dir.dot(&ray_dir);
    let b = ray_dir.dot(&line_dir);
    let c = line_dir.dot(&line_dir);
    let d = ray_dir.dot(&w);
    let e = line_dir.dot(&w);

    let denom = a * c - b * b;
    if denom.abs() < 1e-10 {
        return None;
    }

    let s = (b * e - c * d) / denom;
    let t = (a * e - b * d) / denom;

    let p1 = ray_origin + ray_dir * s;
    let p2 = line_point + line_dir * t;

    Some((p1 - p2).magnitude())
}

fn ray_plane_intersection(
    ray_origin: Point3<f64>,
    ray_dir: Vector3<f64>,
    plane_point: Point3<f64>,
    plane_normal: Vector3<f64>,
) -> Option<f64> {
    let denom = plane_normal.dot(&ray_dir);
    if denom.abs() < 1e-10 {
        return None;
    }

    let t = (plane_point - ray_origin).dot(&plane_normal) / denom;
    if t >= 0.0 {
        Some(t)
    } else {
        None
    }
}

fn closest_point_on_line_to_ray(
    line_point: Point3<f64>,
    line_dir: Vector3<f64>,
    ray_origin: Point3<f64>,
    ray_dir: Vector3<f64>,
) -> Option<Point3<f64>> {
    let w = line_point - ray_origin;
    let a = line_dir.dot(&line_dir);
    let b = line_dir.dot(&ray_dir);
    let c = ray_dir.dot(&ray_dir);
    let d = line_dir.dot(&w);
    let e = ray_dir.dot(&w);

    let denom = a * c - b * b;
    if denom.abs() < 1e-10 {
        return None;
    }

    let t = (b * e - c * d) / denom;
    Some(line_point + line_dir * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gizmo_creation() {
        let gizmo = Gizmo::new();
        assert_eq!(gizmo.mode, GizmoMode::Translate);
        assert_eq!(gizmo.space, GizmoSpace::World);
    }

    #[test]
    fn test_axis_direction() {
        let gizmo = Gizmo::new();

        let x_dir = gizmo.axis_direction(GizmoAxis::X).unwrap();
        assert!((x_dir.x - 1.0).abs() < 1e-10);
        assert!(x_dir.y.abs() < 1e-10);
        assert!(x_dir.z.abs() < 1e-10);
    }

    #[test]
    fn test_plane_normal() {
        let gizmo = Gizmo::new();

        let xy_normal = gizmo.plane_normal(GizmoAxis::XY).unwrap();
        assert!((xy_normal.z - 1.0).abs() < 1e-10);
    }
}
