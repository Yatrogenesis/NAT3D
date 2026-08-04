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

//! Shrinkwrap modifier.
//!
//! Projects vertices onto target mesh surface using various projection modes.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Shrinkwrap projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkwrapMode {
    /// Find nearest point on target surface.
    NearestSurface,
    /// Project along axis.
    Project,
    /// Find nearest vertex on target.
    NearestVertex,
    /// Project along target normals.
    TargetNormal,
}

/// Projection axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectAxis {
    X,
    Y,
    Z,
    ViewAxis,
}

/// Shrinkwrap modifier.
#[derive(Clone)]
pub struct ShrinkwrapModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Target mesh to project onto.
    pub target: ModifierMesh,
    /// Shrinkwrap mode.
    pub mode: ShrinkwrapMode,
    /// Offset from target surface.
    pub offset: f64,
    /// Projection axis (for Project mode).
    pub axis: ProjectAxis,
    /// Use positive axis direction only.
    pub axis_positive: bool,
    /// Use negative axis direction only.
    pub axis_negative: bool,
    /// Vertex group for selective wrapping.
    pub vertex_group: Option<String>,
    /// Auxiliary target mesh (for bi-directional projection).
    pub auxiliary_target: Option<ModifierMesh>,
    /// Snap mode: move to surface or constrain.
    pub snap_mode: bool,
}

impl Default for ShrinkwrapModifier {
    fn default() -> Self {
        Self {
            name: "Shrinkwrap".to_string(),
            enabled: true,
            target: ModifierMesh::new(),
            mode: ShrinkwrapMode::NearestSurface,
            offset: 0.0,
            axis: ProjectAxis::Z,
            axis_positive: false,
            axis_negative: false,
            vertex_group: None,
            auxiliary_target: None,
            snap_mode: false,
        }
    }
}

impl ShrinkwrapModifier {
    /// Create new shrinkwrap modifier with target mesh.
    pub fn new(target: ModifierMesh) -> Self {
        Self {
            target,
            ..Default::default()
        }
    }

    /// Create shrinkwrap with mode.
    pub fn with_mode(target: ModifierMesh, mode: ShrinkwrapMode) -> Self {
        Self {
            target,
            mode,
            ..Default::default()
        }
    }

    /// Find nearest point on target surface.
    fn nearest_surface_point(&self, point: Point3<f64>) -> Option<Point3<f64>> {
        if self.target.faces.is_empty() {
            return None;
        }

        let mut nearest = point;
        let mut min_dist_sq = f64::MAX;

        // Brute force search through all triangles
        for face in &self.target.faces {
            if face.len() < 3 {
                continue;
            }

            // Triangulate face if needed
            for i in 1..face.len() - 1 {
                let v0 = self.target.positions[face[0]];
                let v1 = self.target.positions[face[i]];
                let v2 = self.target.positions[face[i + 1]];

                let closest = closest_point_on_triangle(point, v0, v1, v2);
                let dist_sq = (closest - point).magnitude_squared();

                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                    nearest = closest;
                }
            }
        }

        if min_dist_sq < f64::MAX {
            Some(nearest)
        } else {
            None
        }
    }

    /// Find nearest vertex on target.
    fn nearest_vertex(&self, point: Point3<f64>) -> Option<Point3<f64>> {
        if self.target.positions.is_empty() {
            return None;
        }

        let mut nearest = self.target.positions[0];
        let mut min_dist_sq = (nearest - point).magnitude_squared();

        for &target_pos in &self.target.positions[1..] {
            let dist_sq = (target_pos - point).magnitude_squared();
            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                nearest = target_pos;
            }
        }

        Some(nearest)
    }

    /// Project along axis.
    fn project_along_axis(&self, point: Point3<f64>) -> Option<Point3<f64>> {
        let direction = match self.axis {
            ProjectAxis::X => Vector3::x(),
            ProjectAxis::Y => Vector3::y(),
            ProjectAxis::Z => Vector3::z(),
            ProjectAxis::ViewAxis => Vector3::z(), // Default to Z
        };

        // Ray-triangle intersection
        let mut best_hit: Option<Point3<f64>> = None;
        let mut best_t = f64::MAX;

        for face in &self.target.faces {
            if face.len() < 3 {
                continue;
            }

            for i in 1..face.len() - 1 {
                let v0 = self.target.positions[face[0]];
                let v1 = self.target.positions[face[i]];
                let v2 = self.target.positions[face[i + 1]];

                if let Some((t, hit)) = ray_triangle_intersection(point, direction, v0, v1, v2) {
                    // Check axis direction constraints
                    if self.axis_positive && t < 0.0 {
                        continue;
                    }
                    if self.axis_negative && t > 0.0 {
                        continue;
                    }

                    if t.abs() < best_t.abs() {
                        best_t = t;
                        best_hit = Some(hit);
                    }
                }

                // Also check negative direction if not constrained
                if !self.axis_positive && !self.axis_negative {
                    if let Some((t, hit)) = ray_triangle_intersection(point, -direction, v0, v1, v2) {
                        if t.abs() < best_t.abs() {
                            best_t = t;
                            best_hit = Some(hit);
                        }
                    }
                }
            }
        }

        best_hit
    }

    /// Project along target surface normal.
    fn project_target_normal(&self, point: Point3<f64>) -> Option<Point3<f64>> {
        // First find nearest surface point
        let surface_point = self.nearest_surface_point(point)?;

        // Find the normal at that surface point
        let normal = self.find_surface_normal(surface_point)?;

        // Project along that normal
        Some(surface_point + normal * self.offset)
    }

    /// Find surface normal at given point (approximate).
    fn find_surface_normal(&self, point: Point3<f64>) -> Option<Vector3<f64>> {
        let mut nearest_normal = None;
        let mut min_dist_sq = f64::MAX;

        for face in &self.target.faces {
            if face.len() < 3 {
                continue;
            }

            let v0 = self.target.positions[face[0]];
            let v1 = self.target.positions[face[1]];
            let v2 = self.target.positions[face[2]];

            let center = (v0 + v1.coords + v2.coords) / 3.0;
            let dist_sq = (center - point).magnitude_squared();

            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                let normal = (v1 - v0).cross(&(v2 - v0));
                let len = normal.magnitude();
                if len > 1e-10 {
                    nearest_normal = Some(normal / len);
                }
            }
        }

        nearest_normal
    }

    /// Get vertex weight from vertex group.
    fn get_vertex_weight(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if let Some(ref group_name) = self.vertex_group {
            if let Some(weights) = mesh.vertex_groups.get(group_name) {
                for &(vi, weight) in weights {
                    if vi == vertex_idx {
                        return weight;
                    }
                }
            }
        }
        1.0
    }
}

impl Modifier for ShrinkwrapModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "ShrinkwrapModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.target.positions.is_empty() {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let weight = self.get_vertex_weight(mesh, i);

            if weight < 1e-6 {
                continue;
            }

            let projected = match self.mode {
                ShrinkwrapMode::NearestSurface => self.nearest_surface_point(pos),
                ShrinkwrapMode::Project => self.project_along_axis(pos),
                ShrinkwrapMode::NearestVertex => self.nearest_vertex(pos),
                ShrinkwrapMode::TargetNormal => self.project_target_normal(pos),
            };

            if let Some(mut target_pos) = projected {
                // Apply offset along surface normal if in NearestSurface mode
                if self.mode == ShrinkwrapMode::NearestSurface && self.offset.abs() > 1e-10 {
                    if let Some(normal) = self.find_surface_normal(target_pos) {
                        target_pos += normal * self.offset;
                    }
                }

                // Lerp based on weight
                result.positions[i] = Point3::from(
                    pos.coords + (target_pos - pos) * weight
                );
            }
        }

        result.compute_normals();
        result
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn clone_box(&self) -> Box<dyn Modifier> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Find closest point on triangle to given point.
fn closest_point_on_triangle(
    point: Point3<f64>,
    v0: Point3<f64>,
    v1: Point3<f64>,
    v2: Point3<f64>,
) -> Point3<f64> {
    let edge0 = v1 - v0;
    let edge1 = v2 - v0;
    let v0_to_point = point - v0;

    let a = edge0.dot(&edge0);
    let b = edge0.dot(&edge1);
    let c = edge1.dot(&edge1);
    let d = edge0.dot(&v0_to_point);
    let e = edge1.dot(&v0_to_point);

    let det = a * c - b * b;
    let mut s = b * e - c * d;
    let mut t = b * d - a * e;

    if det < 1e-10 {
        // Degenerate triangle
        return v0;
    }

    s /= det;
    t /= det;

    if s < 0.0 {
        s = 0.0;
    }
    if t < 0.0 {
        t = 0.0;
    }
    if s + t > 1.0 {
        let denom = s + t;
        s /= denom;
        t /= denom;
    }

    v0 + edge0 * s + edge1 * t
}

/// Ray-triangle intersection (Möller-Trumbore algorithm).
fn ray_triangle_intersection(
    origin: Point3<f64>,
    direction: Vector3<f64>,
    v0: Point3<f64>,
    v1: Point3<f64>,
    v2: Point3<f64>,
) -> Option<(f64, Point3<f64>)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = direction.cross(&edge2);
    let a = edge1.dot(&h);

    if a.abs() < 1e-10 {
        return None; // Ray parallel to triangle
    }

    let f = 1.0 / a;
    let s = origin - v0;
    let u = f * s.dot(&h);

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(&edge1);
    let v = f * direction.dot(&q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(&q);
    let hit = origin + direction * t;

    Some((t, hit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shrinkwrap_nearest_surface() {
        // Create a simple target plane
        let target = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(-1.0, 0.0, 1.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        // Create source mesh above the plane
        let source = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 2.0, 0.0),
                Point3::new(0.5, 3.0, 0.0),
            ],
            vec![],
        );

        let modifier = ShrinkwrapModifier::new(target);
        let result = modifier.apply(&source);

        // Vertices should be projected onto the plane (y = 0)
        assert_eq!(result.positions.len(), 2);
        assert!((result.positions[0].y).abs() < 0.1);
        assert!((result.positions[1].y).abs() < 0.1);
    }

    #[test]
    fn test_shrinkwrap_nearest_vertex() {
        let target = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let source = ModifierMesh::from_geometry(
            vec![Point3::new(0.1, 0.1, 0.0)],
            vec![],
        );

        let modifier = ShrinkwrapModifier::with_mode(target, ShrinkwrapMode::NearestVertex);
        let result = modifier.apply(&source);

        // Should snap to origin (nearest vertex)
        assert_eq!(result.positions.len(), 1);
        assert!((result.positions[0] - Point3::origin()).magnitude() < 0.2);
    }

    #[test]
    fn test_shrinkwrap_with_offset() {
        let target = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(-1.0, 0.0, 1.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let source = ModifierMesh::from_geometry(
            vec![Point3::new(0.0, 2.0, 0.0)],
            vec![],
        );

        let mut modifier = ShrinkwrapModifier::new(target);
        modifier.offset = 0.5;

        let result = modifier.apply(&source);

        // Should be projected and offset from surface
        assert_eq!(result.positions.len(), 1);
        // Should be moved closer to target (distance reduced)
        let original_dist = (source.positions[0] - Point3::origin()).magnitude();
        let result_dist = (result.positions[0] - Point3::origin()).magnitude();
        assert!(result_dist < original_dist); // Moved closer to surface
    }
}
