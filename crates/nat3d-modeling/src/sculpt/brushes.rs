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

//! Sculpt brushes.
//!
//! Implements various sculpting brushes for mesh deformation including
//! standard, clay, grab, smooth, pinch, and other brush types.

use nalgebra::{Point3, Vector3};
use std::collections::HashSet;

/// Brush type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushType {
    /// Standard draw brush - pushes vertices along normal.
    Draw,
    /// Clay brush - adds volume like clay.
    Clay,
    /// Clay strips brush - adds strips of clay.
    ClayStrips,
    /// Inflate brush - pushes vertices along their individual normals.
    Inflate,
    /// Flatten brush - flattens to average plane.
    Flatten,
    /// Fill brush - fills concave areas.
    Fill,
    /// Scrape brush - removes convex areas.
    Scrape,
    /// Smooth brush - averages vertex positions.
    Smooth,
    /// Pinch brush - pulls vertices toward center.
    Pinch,
    /// Grab brush - moves vertices with stroke.
    Grab,
    /// Snake hook - pulls mesh like a hook.
    SnakeHook,
    /// Crease brush - creates sharp creases.
    Crease,
    /// Blob brush - adds spherical volume.
    Blob,
    /// Layer brush - adds uniform layers.
    Layer,
    /// Mask brush - paints mask values.
    Mask,
}

/// Brush falloff type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloffType {
    /// Smooth falloff (default).
    Smooth,
    /// Sharp falloff - more abrupt edge.
    Sharp,
    /// Linear falloff.
    Linear,
    /// Sphere falloff - constant within radius.
    Sphere,
    /// Custom curve falloff.
    Custom,
}

/// Brush stroke type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeType {
    /// Standard dragging stroke.
    Drag,
    /// Dot stroke - single application.
    Dot,
    /// Anchored stroke - drags from initial point.
    Anchored,
    /// Airbrush - continuous application over time.
    Airbrush,
    /// Line stroke - straight line between points.
    Line,
    /// Curve stroke - follows a curve.
    Curve,
}

/// Sculpting brush configuration.
#[derive(Debug, Clone)]
pub struct Brush {
    /// Brush type.
    pub brush_type: BrushType,
    /// Brush radius in world units.
    pub radius: f64,
    /// Brush strength (0.0 to 1.0).
    pub strength: f64,
    /// Falloff type.
    pub falloff: FalloffType,
    /// Custom falloff curve (sampled).
    pub falloff_curve: Vec<f64>,
    /// Stroke type.
    pub stroke_type: StrokeType,
    /// Whether to use front faces only.
    pub front_faces_only: bool,
    /// Texture for brush alpha.
    pub texture: Option<BrushTexture>,
    /// Auto-smooth amount after stroke.
    pub auto_smooth: f64,
    /// Normal weight for draw direction.
    pub normal_weight: f64,
    /// Plane offset for flatten/fill/scrape.
    pub plane_offset: f64,
    /// Whether brush direction is inverted.
    pub invert: bool,
}

/// Brush texture for alpha masking.
#[derive(Debug, Clone)]
pub struct BrushTexture {
    /// Texture data (grayscale).
    pub data: Vec<f64>,
    /// Texture width.
    pub width: usize,
    /// Texture height.
    pub height: usize,
    /// Texture rotation in radians.
    pub rotation: f64,
    /// Texture scale.
    pub scale: f64,
}

/// Brush stroke data.
#[derive(Debug, Clone)]
pub struct BrushStroke {
    /// Stroke points.
    pub points: Vec<StrokePoint>,
    /// Total stroke length.
    pub length: f64,
}

/// A single point in a brush stroke.
#[derive(Debug, Clone)]
pub struct StrokePoint {
    /// Position on surface.
    pub position: Point3<f64>,
    /// Surface normal at point.
    pub normal: Vector3<f64>,
    /// Brush pressure (0.0 to 1.0).
    pub pressure: f64,
    /// Brush tilt (radians).
    pub tilt: f64,
}

/// Mesh data for sculpting.
pub struct SculptMesh {
    /// Vertex positions.
    pub positions: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices.
    pub indices: Vec<[u32; 3]>,
    /// Mask values per vertex.
    pub mask: Vec<f64>,
    /// Vertex connectivity (adjacent vertices).
    pub adjacency: Vec<Vec<usize>>,
    /// Original positions for layer brush.
    pub layer_base: Option<Vec<Point3<f64>>>,
    /// Layer displacement tracking.
    pub layer_displacement: Vec<f64>,
}

/// Result of a brush application.
pub struct BrushResult {
    /// Indices of affected vertices.
    pub affected_vertices: Vec<usize>,
    /// New positions for affected vertices.
    pub new_positions: Vec<Point3<f64>>,
    /// Whether normals need recalculation.
    pub needs_normal_update: bool,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            brush_type: BrushType::Draw,
            radius: 0.1,
            strength: 0.5,
            falloff: FalloffType::Smooth,
            falloff_curve: vec![1.0, 0.9, 0.7, 0.4, 0.1, 0.0],
            stroke_type: StrokeType::Drag,
            front_faces_only: true,
            texture: None,
            auto_smooth: 0.0,
            normal_weight: 1.0,
            plane_offset: 0.0,
            invert: false,
        }
    }
}

impl Brush {
    /// Create a new draw brush.
    pub fn draw(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Draw,
            radius,
            strength,
            ..Default::default()
        }
    }

    /// Create a new clay brush.
    pub fn clay(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Clay,
            radius,
            strength,
            normal_weight: 0.5,
            ..Default::default()
        }
    }

    /// Create a new smooth brush.
    pub fn smooth(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Smooth,
            radius,
            strength,
            front_faces_only: false,
            ..Default::default()
        }
    }

    /// Create a new grab brush.
    pub fn grab(radius: f64) -> Self {
        Self {
            brush_type: BrushType::Grab,
            radius,
            strength: 1.0,
            stroke_type: StrokeType::Drag,
            front_faces_only: false,
            ..Default::default()
        }
    }

    /// Create a new flatten brush.
    pub fn flatten(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Flatten,
            radius,
            strength,
            ..Default::default()
        }
    }

    /// Create a new inflate brush.
    pub fn inflate(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Inflate,
            radius,
            strength,
            ..Default::default()
        }
    }

    /// Create a new pinch brush.
    pub fn pinch(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Pinch,
            radius,
            strength,
            ..Default::default()
        }
    }

    /// Create a new crease brush.
    pub fn crease(radius: f64, strength: f64) -> Self {
        Self {
            brush_type: BrushType::Crease,
            radius,
            strength,
            ..Default::default()
        }
    }

    /// Create a new mask brush.
    pub fn mask(radius: f64) -> Self {
        Self {
            brush_type: BrushType::Mask,
            radius,
            strength: 1.0,
            front_faces_only: false,
            ..Default::default()
        }
    }

    /// Calculate falloff for a given distance ratio (0.0 at center, 1.0 at edge).
    pub fn calculate_falloff(&self, distance_ratio: f64) -> f64 {
        if distance_ratio >= 1.0 {
            return 0.0;
        }
        if distance_ratio <= 0.0 {
            return 1.0;
        }

        match self.falloff {
            FalloffType::Smooth => {
                // Smooth hermite interpolation
                let t = distance_ratio;
                let t2 = t * t;
                let t3 = t2 * t;
                1.0 - (3.0 * t2 - 2.0 * t3)
            }
            FalloffType::Sharp => {
                let t = 1.0 - distance_ratio;
                t * t * t
            }
            FalloffType::Linear => 1.0 - distance_ratio,
            FalloffType::Sphere => 1.0,
            FalloffType::Custom => {
                // Sample the custom curve
                if self.falloff_curve.is_empty() {
                    return 1.0 - distance_ratio;
                }
                let index = (distance_ratio * (self.falloff_curve.len() - 1) as f64) as usize;
                let frac = distance_ratio * (self.falloff_curve.len() - 1) as f64 - index as f64;
                let v0 = self.falloff_curve[index.min(self.falloff_curve.len() - 1)];
                let v1 = self.falloff_curve[(index + 1).min(self.falloff_curve.len() - 1)];
                v0 * (1.0 - frac) + v1 * frac
            }
        }
    }

    /// Apply brush at a single stroke point.
    pub fn apply(&self, mesh: &mut SculptMesh, point: &StrokePoint) -> BrushResult {
        let mut affected = Vec::new();
        let mut new_positions = Vec::new();

        // Find vertices within radius
        let affected_indices: Vec<usize> = mesh
            .positions
            .iter()
            .enumerate()
            .filter_map(|(i, pos)| {
                let dist = (pos - point.position).norm();
                if dist <= self.radius {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        // Filter by mask
        let affected_indices: Vec<usize> = affected_indices
            .into_iter()
            .filter(|&i| mesh.mask[i] < 1.0)
            .collect();

        // Calculate displacement direction based on brush type
        let direction = if self.invert { -1.0 } else { 1.0 };
        let strength = self.strength * point.pressure * direction;

        match self.brush_type {
            BrushType::Draw => {
                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    let displacement =
                        point.normal * (strength * falloff * mask_factor * self.radius * 0.1);
                    let new_pos = pos + displacement;

                    affected.push(i);
                    new_positions.push(new_pos);
                }
            }
            BrushType::Clay => {
                // Calculate average plane
                let avg_height = self.calculate_plane_height(
                    mesh,
                    &affected_indices,
                    &point.normal,
                    point.position,
                );

                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    let height = (pos - point.position).dot(&point.normal);
                    let target_height = avg_height + strength * self.radius * 0.1;

                    if (strength > 0.0 && height < target_height)
                        || (strength < 0.0 && height > target_height)
                    {
                        let diff = (target_height - height) * falloff * mask_factor;
                        let displacement = point.normal * diff;
                        let new_pos = pos + displacement;

                        affected.push(i);
                        new_positions.push(new_pos);
                    }
                }
            }
            BrushType::Inflate => {
                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let normal = mesh.normals[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    let displacement =
                        normal * (strength * falloff * mask_factor * self.radius * 0.1);
                    let new_pos = pos + displacement;

                    affected.push(i);
                    new_positions.push(new_pos);
                }
            }
            BrushType::Smooth => {
                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    // Calculate average of neighbors
                    let neighbors = &mesh.adjacency[i];
                    if neighbors.is_empty() {
                        continue;
                    }

                    let avg: Point3<f64> = neighbors
                        .iter()
                        .map(|&n| mesh.positions[n])
                        .fold(Point3::<f64>::origin(), |acc, p| {
                            Point3::new(acc.x + p.x, acc.y + p.y, acc.z + p.z)
                        });
                    let avg = Point3::new(
                        avg.x / neighbors.len() as f64,
                        avg.y / neighbors.len() as f64,
                        avg.z / neighbors.len() as f64,
                    );

                    let blend = strength.abs() * falloff * mask_factor;
                    let new_pos = Point3::new(
                        pos.x * (1.0 - blend) + avg.x * blend,
                        pos.y * (1.0 - blend) + avg.y * blend,
                        pos.z * (1.0 - blend) + avg.z * blend,
                    );

                    affected.push(i);
                    new_positions.push(new_pos);
                }
            }
            BrushType::Flatten => {
                let avg_height = self.calculate_plane_height(
                    mesh,
                    &affected_indices,
                    &point.normal,
                    point.position,
                );
                let target_height = avg_height + self.plane_offset;

                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    let height = (pos - point.position).dot(&point.normal);
                    let diff = (target_height - height) * strength.abs() * falloff * mask_factor;
                    let displacement = point.normal * diff;
                    let new_pos = pos + displacement;

                    affected.push(i);
                    new_positions.push(new_pos);
                }
            }
            BrushType::Pinch => {
                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    // Direction toward center, projected onto tangent plane
                    let to_center = point.position - pos;
                    let tangent_dir = to_center - point.normal * to_center.dot(&point.normal);
                    let tangent_dist = tangent_dir.norm();

                    if tangent_dist > 1e-10 {
                        let dir = tangent_dir.normalize();
                        let displacement =
                            dir * (strength * falloff * mask_factor * tangent_dist * 0.1);
                        let new_pos = pos + displacement;

                        affected.push(i);
                        new_positions.push(new_pos);
                    }
                }
            }
            BrushType::Grab => {
                // Grab doesn't work with single point, needs delta from previous
                // This is handled at stroke level
            }
            BrushType::Crease => {
                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    // Combine pinch and draw
                    let to_center = point.position - pos;
                    let tangent_dir = to_center - point.normal * to_center.dot(&point.normal);
                    let tangent_dist = tangent_dir.norm();

                    let mut displacement =
                        point.normal * (strength * falloff * mask_factor * self.radius * 0.05);

                    if tangent_dist > 1e-10 {
                        let dir = tangent_dir.normalize();
                        displacement +=
                            dir * (strength * falloff * mask_factor * tangent_dist * 0.05);
                    }

                    let new_pos = pos + displacement;
                    affected.push(i);
                    new_positions.push(new_pos);
                }
            }
            BrushType::Layer => {
                if mesh.layer_base.is_none() {
                    // Initialize layer base
                    mesh.layer_base = Some(mesh.positions.clone());
                    mesh.layer_displacement = vec![0.0; mesh.positions.len()];
                }

                let base = mesh.layer_base.as_ref().unwrap();

                for &i in &affected_indices {
                    let pos = base[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);
                    let mask_factor = 1.0 - mesh.mask[i];

                    let target_disp = strength * self.radius * 0.1;
                    let current_disp = mesh.layer_displacement[i];

                    if (target_disp > 0.0 && current_disp < target_disp)
                        || (target_disp < 0.0 && current_disp > target_disp)
                    {
                        let new_disp = if target_disp > 0.0 {
                            current_disp.max(target_disp * falloff * mask_factor)
                        } else {
                            current_disp.min(target_disp * falloff * mask_factor)
                        };

                        mesh.layer_displacement[i] = new_disp;
                        let new_pos = pos + point.normal * new_disp;

                        affected.push(i);
                        new_positions.push(new_pos);
                    }
                }
            }
            BrushType::Mask => {
                for &i in &affected_indices {
                    let pos = mesh.positions[i];
                    let dist = (pos - point.position).norm();
                    let falloff = self.calculate_falloff(dist / self.radius);

                    let mask_delta = if self.invert { -falloff } else { falloff };
                    mesh.mask[i] = (mesh.mask[i] + mask_delta * strength).clamp(0.0, 1.0);
                }
                // Mask doesn't change positions
            }
            _ => {}
        }

        let needs_update = !affected.is_empty() && self.brush_type != BrushType::Mask;
        BrushResult {
            affected_vertices: affected,
            new_positions,
            needs_normal_update: needs_update,
        }
    }

    /// Apply grab brush with delta movement.
    pub fn apply_grab(
        &self,
        mesh: &mut SculptMesh,
        initial_point: Point3<f64>,
        delta: Vector3<f64>,
    ) -> BrushResult {
        let mut affected = Vec::new();
        let mut new_positions = Vec::new();

        for (i, pos) in mesh.positions.iter().enumerate() {
            let dist = (pos - initial_point).norm();
            if dist <= self.radius && mesh.mask[i] < 1.0 {
                let falloff = self.calculate_falloff(dist / self.radius);
                let mask_factor = 1.0 - mesh.mask[i];

                let displacement = delta * (falloff * mask_factor);
                let new_pos = pos + displacement;

                affected.push(i);
                new_positions.push(new_pos);
            }
        }

        BrushResult {
            affected_vertices: affected,
            new_positions,
            needs_normal_update: true,
        }
    }

    fn calculate_plane_height(
        &self,
        mesh: &SculptMesh,
        indices: &[usize],
        normal: &Vector3<f64>,
        center: Point3<f64>,
    ) -> f64 {
        if indices.is_empty() {
            return 0.0;
        }

        let sum: f64 = indices
            .iter()
            .map(|&i| (mesh.positions[i] - center).dot(normal))
            .sum();

        sum / indices.len() as f64
    }
}

impl SculptMesh {
    /// Create a new sculpt mesh from positions and indices.
    pub fn new(positions: Vec<Point3<f64>>, indices: Vec<[u32; 3]>) -> Self {
        let n = positions.len();
        let mut adjacency = vec![Vec::new(); n];

        // Build adjacency from triangles
        for tri in &indices {
            let a = tri[0] as usize;
            let b = tri[1] as usize;
            let c = tri[2] as usize;

            if !adjacency[a].contains(&b) {
                adjacency[a].push(b);
            }
            if !adjacency[a].contains(&c) {
                adjacency[a].push(c);
            }
            if !adjacency[b].contains(&a) {
                adjacency[b].push(a);
            }
            if !adjacency[b].contains(&c) {
                adjacency[b].push(c);
            }
            if !adjacency[c].contains(&a) {
                adjacency[c].push(a);
            }
            if !adjacency[c].contains(&b) {
                adjacency[c].push(b);
            }
        }

        // Compute normals
        let normals = Self::compute_normals(&positions, &indices);

        Self {
            positions,
            normals,
            indices,
            mask: vec![0.0; n],
            adjacency,
            layer_base: None,
            layer_displacement: vec![0.0; n],
        }
    }

    /// Compute vertex normals from triangle faces.
    pub fn compute_normals(positions: &[Point3<f64>], indices: &[[u32; 3]]) -> Vec<Vector3<f64>> {
        let mut normals = vec![Vector3::zeros(); positions.len()];

        for tri in indices {
            let a = tri[0] as usize;
            let b = tri[1] as usize;
            let c = tri[2] as usize;

            let v0 = positions[a];
            let v1 = positions[b];
            let v2 = positions[c];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let face_normal = edge1.cross(&edge2);

            normals[a] += face_normal;
            normals[b] += face_normal;
            normals[c] += face_normal;
        }

        for n in &mut normals {
            let len = n.norm();
            if len > 1e-10 {
                *n /= len;
            }
        }

        normals
    }

    /// Update normals for specific vertices.
    pub fn update_normals(&mut self, affected: &[usize]) {
        // Collect affected triangles
        let mut affected_set: HashSet<usize> = HashSet::new();
        for &v in affected {
            affected_set.insert(v);
        }

        // Reset normals for affected vertices
        for &v in affected {
            self.normals[v] = Vector3::zeros();
        }

        // Accumulate face normals
        for tri in &self.indices {
            let a = tri[0] as usize;
            let b = tri[1] as usize;
            let c = tri[2] as usize;

            if affected_set.contains(&a) || affected_set.contains(&b) || affected_set.contains(&c) {
                let v0 = self.positions[a];
                let v1 = self.positions[b];
                let v2 = self.positions[c];

                let edge1 = v1 - v0;
                let edge2 = v2 - v0;
                let face_normal = edge1.cross(&edge2);

                if affected_set.contains(&a) {
                    self.normals[a] += face_normal;
                }
                if affected_set.contains(&b) {
                    self.normals[b] += face_normal;
                }
                if affected_set.contains(&c) {
                    self.normals[c] += face_normal;
                }
            }
        }

        // Normalize
        for &v in affected {
            let len = self.normals[v].norm();
            if len > 1e-10 {
                self.normals[v] /= len;
            }
        }
    }

    /// Apply brush result to mesh.
    pub fn apply_result(&mut self, result: &BrushResult) {
        for (i, &idx) in result.affected_vertices.iter().enumerate() {
            self.positions[idx] = result.new_positions[i];
        }

        if result.needs_normal_update {
            self.update_normals(&result.affected_vertices);
        }
    }

    /// Clear mask values.
    pub fn clear_mask(&mut self) {
        self.mask.fill(0.0);
    }

    /// Invert mask values.
    pub fn invert_mask(&mut self) {
        for m in &mut self.mask {
            *m = 1.0 - *m;
        }
    }

    /// Reset layer base.
    pub fn reset_layer(&mut self) {
        self.layer_base = None;
        self.layer_displacement.fill(0.0);
    }
}

impl BrushStroke {
    /// Create a new brush stroke.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            length: 0.0,
        }
    }

    /// Add a point to the stroke.
    pub fn add_point(&mut self, point: StrokePoint) {
        if let Some(last) = self.points.last() {
            self.length += (point.position - last.position).norm();
        }
        self.points.push(point);
    }

    /// Check if stroke needs a new dab based on spacing.
    pub fn needs_dab(&self, spacing: f64) -> bool {
        if self.points.len() < 2 {
            return true;
        }

        let last_dist = self.length % spacing;
        last_dist < spacing * 0.5
    }

    /// Get interpolated points at regular spacing.
    pub fn sample_at_spacing(&self, spacing: f64) -> Vec<StrokePoint> {
        if self.points.is_empty() || spacing <= 0.0 {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut accumulated = 0.0;
        let mut target = 0.0;

        for i in 0..self.points.len() - 1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i + 1];
            let segment_len = (p1.position - p0.position).norm();

            while target <= accumulated + segment_len {
                let t = if segment_len > 0.0 {
                    (target - accumulated) / segment_len
                } else {
                    0.0
                };

                // Interpolate stroke point
                let position = Point3::new(
                    p0.position.x * (1.0 - t) + p1.position.x * t,
                    p0.position.y * (1.0 - t) + p1.position.y * t,
                    p0.position.z * (1.0 - t) + p1.position.z * t,
                );
                let normal = (p0.normal * (1.0 - t) + p1.normal * t).normalize();
                let pressure = p0.pressure * (1.0 - t) + p1.pressure * t;
                let tilt = p0.tilt * (1.0 - t) + p1.tilt * t;

                result.push(StrokePoint {
                    position,
                    normal,
                    pressure,
                    tilt,
                });

                target += spacing;
            }

            accumulated += segment_len;
        }

        result
    }
}

impl Default for BrushStroke {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brush_falloff() {
        let brush = Brush::default();

        assert!((brush.calculate_falloff(0.0) - 1.0).abs() < 1e-10);
        assert!(brush.calculate_falloff(0.5) > 0.0);
        assert!(brush.calculate_falloff(0.5) < 1.0);
        assert!(brush.calculate_falloff(1.0).abs() < 1e-10);
    }

    #[test]
    fn test_brush_apply() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];

        let mut mesh = SculptMesh::new(positions, indices);
        let brush = Brush::draw(2.0, 0.5);

        let point = StrokePoint {
            position: Point3::new(0.5, 0.5, 0.0),
            normal: Vector3::new(0.0, 0.0, 1.0),
            pressure: 1.0,
            tilt: 0.0,
        };

        let result = brush.apply(&mut mesh, &point);
        assert!(!result.affected_vertices.is_empty());
    }

    #[test]
    fn test_smooth_brush() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 0.0, 0.5), // Vertex to smooth
            Point3::new(0.5, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2], [1, 3, 2], [0, 2, 3]];

        let mut mesh = SculptMesh::new(positions, indices);
        let brush = Brush::smooth(2.0, 0.5);

        let point = StrokePoint {
            position: Point3::new(0.5, 0.5, 0.0),
            normal: Vector3::new(0.0, 0.0, 1.0),
            pressure: 1.0,
            tilt: 0.0,
        };

        let result = brush.apply(&mut mesh, &point);
        mesh.apply_result(&result);

        // Vertex 2 should have moved toward average of neighbors
        assert!(mesh.positions[2].z < 0.5);
    }
}
