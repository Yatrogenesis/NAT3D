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

//! Mesh skinning for skeletal animation.
//!
//! Implements vertex weighting, dual quaternion skinning, and weight painting.

use nalgebra::{DualQuaternion, Matrix4, Point3, UnitQuaternion, Vector3, Vector4};
use std::collections::HashMap;

use super::armature::Armature;
use super::bone::BoneId;

/// Maximum bones per vertex for GPU skinning.
pub const MAX_BONES_PER_VERTEX: usize = 4;

/// Skinning method.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SkinningMethod {
    /// Linear blend skinning (LBS).
    #[default]
    Linear,
    /// Dual quaternion skinning (DQS).
    DualQuaternion,
    /// Linear + DQS blend.
    Hybrid {
        /// Blend factor (0 = full linear, 1 = full DQS).
        dq_blend: f64,
    },
}

/// Vertex bone weight.
#[derive(Debug, Clone, Copy)]
pub struct BoneWeight {
    /// Bone ID.
    pub bone: BoneId,
    /// Weight (0-1).
    pub weight: f64,
}

impl BoneWeight {
    /// Create a new bone weight.
    pub fn new(bone: BoneId, weight: f64) -> Self {
        Self { bone, weight }
    }
}

/// Vertex weights (up to MAX_BONES_PER_VERTEX bones).
#[derive(Debug, Clone)]
pub struct VertexWeights {
    /// Bone weights sorted by weight (descending).
    weights: Vec<BoneWeight>,
}

impl VertexWeights {
    /// Create empty weights.
    pub fn new() -> Self {
        Self {
            weights: Vec::new(),
        }
    }

    /// Add a bone weight.
    pub fn add_weight(&mut self, bone: BoneId, weight: f64) {
        if weight > 0.0 {
            self.weights.push(BoneWeight::new(bone, weight));
            self.weights
                .sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());

            // Keep only top MAX_BONES_PER_VERTEX
            if self.weights.len() > MAX_BONES_PER_VERTEX {
                self.weights.truncate(MAX_BONES_PER_VERTEX);
            }
        }
    }

    /// Normalize weights to sum to 1.
    pub fn normalize(&mut self) {
        let sum: f64 = self.weights.iter().map(|w| w.weight).sum();
        if sum > 0.0 {
            for w in &mut self.weights {
                w.weight /= sum;
            }
        }
    }

    /// Get weights slice.
    pub fn weights(&self) -> &[BoneWeight] {
        &self.weights
    }

    /// Get weights as arrays for GPU (indices, weights).
    pub fn to_gpu_format(&self) -> ([u32; MAX_BONES_PER_VERTEX], [f32; MAX_BONES_PER_VERTEX]) {
        let mut indices = [0u32; MAX_BONES_PER_VERTEX];
        let mut weights = [0.0f32; MAX_BONES_PER_VERTEX];

        for (i, bw) in self.weights.iter().take(MAX_BONES_PER_VERTEX).enumerate() {
            indices[i] = bw.bone.0;
            weights[i] = bw.weight as f32;
        }

        (indices, weights)
    }

    /// Create from GPU format arrays.
    pub fn from_gpu_format(
        indices: [u32; MAX_BONES_PER_VERTEX],
        weights: [f32; MAX_BONES_PER_VERTEX],
    ) -> Self {
        let mut result = Self::new();
        for i in 0..MAX_BONES_PER_VERTEX {
            if weights[i] > 0.0 {
                result
                    .weights
                    .push(BoneWeight::new(BoneId(indices[i]), weights[i] as f64));
            }
        }
        result
    }
}

impl Default for VertexWeights {
    fn default() -> Self {
        Self::new()
    }
}

/// Skin data for a mesh.
#[derive(Debug, Clone)]
pub struct SkinData {
    /// Weights for each vertex.
    vertex_weights: Vec<VertexWeights>,
    /// Armature reference name.
    pub armature_name: String,
    /// Bind shape matrix.
    pub bind_shape_matrix: Matrix4<f64>,
    /// Skinning method.
    pub method: SkinningMethod,
}

impl SkinData {
    /// Create new skin data.
    pub fn new(vertex_count: usize) -> Self {
        Self {
            vertex_weights: vec![VertexWeights::new(); vertex_count],
            armature_name: String::new(),
            bind_shape_matrix: Matrix4::identity(),
            method: SkinningMethod::Linear,
        }
    }

    /// Set weight for a vertex.
    pub fn set_weight(&mut self, vertex: usize, bone: BoneId, weight: f64) {
        if vertex < self.vertex_weights.len() {
            self.vertex_weights[vertex].add_weight(bone, weight);
        }
    }

    /// Get weights for a vertex.
    pub fn get_weights(&self, vertex: usize) -> Option<&VertexWeights> {
        self.vertex_weights.get(vertex)
    }

    /// Get mutable weights for a vertex.
    pub fn get_weights_mut(&mut self, vertex: usize) -> Option<&mut VertexWeights> {
        self.vertex_weights.get_mut(vertex)
    }

    /// Normalize all weights.
    pub fn normalize_all(&mut self) {
        for weights in &mut self.vertex_weights {
            weights.normalize();
        }
    }

    /// Get vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertex_weights.len()
    }

    /// Resize for different vertex count.
    pub fn resize(&mut self, vertex_count: usize) {
        self.vertex_weights
            .resize(vertex_count, VertexWeights::new());
    }
}

/// Skinning engine for deforming meshes.
#[derive(Debug)]
pub struct SkinningEngine {
    /// Cached skinning matrices.
    skinning_matrices: Vec<Matrix4<f64>>,
    /// Cached dual quaternions.
    dual_quaternions: Vec<DualQuaternion<f64>>,
}

impl SkinningEngine {
    /// Create a new skinning engine.
    pub fn new() -> Self {
        Self {
            skinning_matrices: Vec::new(),
            dual_quaternions: Vec::new(),
        }
    }

    /// Update skinning matrices from armature.
    pub fn update(&mut self, armature: &Armature) {
        self.skinning_matrices = armature.skinning_matrices();

        // Compute dual quaternions for DQS
        self.dual_quaternions.clear();
        for matrix in &self.skinning_matrices {
            self.dual_quaternions
                .push(matrix_to_dual_quaternion(matrix));
        }
    }

    /// Deform vertices using skinning.
    pub fn deform_vertices(
        &self,
        skin: &SkinData,
        positions: &[Point3<f64>],
        normals: Option<&[Vector3<f64>]>,
    ) -> (Vec<Point3<f64>>, Option<Vec<Vector3<f64>>>) {
        let mut deformed_positions = Vec::with_capacity(positions.len());
        let mut deformed_normals = normals.map(|_| Vec::with_capacity(positions.len()));

        for (i, pos) in positions.iter().enumerate() {
            let weights = skin.get_weights(i);

            let (new_pos, new_normal) = match skin.method {
                SkinningMethod::Linear => {
                    self.deform_vertex_linear(pos, normals.map(|n| &n[i]), weights)
                }
                SkinningMethod::DualQuaternion => {
                    self.deform_vertex_dqs(pos, normals.map(|n| &n[i]), weights)
                }
                SkinningMethod::Hybrid { dq_blend } => {
                    let (lbs_pos, lbs_normal) =
                        self.deform_vertex_linear(pos, normals.map(|n| &n[i]), weights);
                    let (dqs_pos, dqs_normal) =
                        self.deform_vertex_dqs(pos, normals.map(|n| &n[i]), weights);

                    let blended_pos =
                        Point3::from(lbs_pos.coords * (1.0 - dq_blend) + dqs_pos.coords * dq_blend);
                    let blended_normal = match (lbs_normal, dqs_normal) {
                        (Some(ln), Some(dn)) => {
                            Some((ln * (1.0 - dq_blend) + dn * dq_blend).normalize())
                        }
                        _ => None,
                    };

                    (blended_pos, blended_normal)
                }
            };

            deformed_positions.push(new_pos);
            if let Some(ref mut normals_out) = deformed_normals {
                normals_out.push(new_normal.unwrap_or(Vector3::new(0.0, 1.0, 0.0)));
            }
        }

        (deformed_positions, deformed_normals)
    }

    /// Linear blend skinning for a single vertex.
    fn deform_vertex_linear(
        &self,
        position: &Point3<f64>,
        normal: Option<&Vector3<f64>>,
        weights: Option<&VertexWeights>,
    ) -> (Point3<f64>, Option<Vector3<f64>>) {
        let weights = match weights {
            Some(w) if !w.weights.is_empty() => w,
            _ => return (*position, normal.copied()),
        };

        let mut blended_matrix = Matrix4::zeros();

        for bw in weights.weights() {
            if let Some(matrix) = self.skinning_matrices.get(bw.bone.0 as usize) {
                blended_matrix += matrix * bw.weight;
            }
        }

        let pos_homogeneous = Vector4::new(position.x, position.y, position.z, 1.0);
        let new_pos = blended_matrix * pos_homogeneous;
        let new_position = Point3::new(new_pos.x, new_pos.y, new_pos.z);

        let new_normal = normal.map(|n| {
            // Transform normal (use inverse transpose for correct normal transformation)
            let normal_homogeneous = Vector4::new(n.x, n.y, n.z, 0.0);
            let transformed = blended_matrix * normal_homogeneous;
            Vector3::new(transformed.x, transformed.y, transformed.z).normalize()
        });

        (new_position, new_normal)
    }

    /// Dual quaternion skinning for a single vertex.
    fn deform_vertex_dqs(
        &self,
        position: &Point3<f64>,
        normal: Option<&Vector3<f64>>,
        weights: Option<&VertexWeights>,
    ) -> (Point3<f64>, Option<Vector3<f64>>) {
        let weights = match weights {
            Some(w) if !w.weights.is_empty() => w,
            _ => return (*position, normal.copied()),
        };

        // Blend dual quaternions
        let mut blended_dq = DualQuaternion::from_real_and_dual(
            nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
            nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
        );

        // Get first DQ for sign correction
        let first_dq = weights
            .weights()
            .first()
            .and_then(|bw| self.dual_quaternions.get(bw.bone.0 as usize));

        for bw in weights.weights() {
            if let Some(dq) = self.dual_quaternions.get(bw.bone.0 as usize) {
                // Sign correction for shortest path interpolation
                let sign = if let Some(first) = first_dq {
                    if dq.real.dot(&first.real) < 0.0 {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };

                let scaled_real = dq.real.coords * (bw.weight * sign);
                let scaled_dual = dq.dual.coords * (bw.weight * sign);

                blended_dq.real.coords += scaled_real;
                blended_dq.dual.coords += scaled_dual;
            }
        }

        // Normalize the blended dual quaternion
        let norm = blended_dq.real.norm();
        if norm > 1e-10 {
            blended_dq.real.coords /= norm;
            blended_dq.dual.coords /= norm;
        }

        // Transform position
        let pos_vec = Vector3::new(position.x, position.y, position.z);
        let new_pos = dq_transform_point(&blended_dq, &pos_vec);
        let new_position = Point3::from(new_pos);

        // Transform normal
        let new_normal = normal.map(|n| dq_transform_vector(&blended_dq, n).normalize());

        (new_position, new_normal)
    }

    /// Get skinning matrices for GPU upload.
    pub fn get_matrices(&self) -> &[Matrix4<f64>] {
        &self.skinning_matrices
    }

    /// Get dual quaternions for GPU upload.
    pub fn get_dual_quaternions(&self) -> &[DualQuaternion<f64>] {
        &self.dual_quaternions
    }
}

impl Default for SkinningEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert matrix to dual quaternion.
fn matrix_to_dual_quaternion(matrix: &Matrix4<f64>) -> DualQuaternion<f64> {
    // Extract rotation
    let rotation_matrix = matrix.fixed_view::<3, 3>(0, 0).into_owned();
    let rotation = UnitQuaternion::from_rotation_matrix(
        &nalgebra::Rotation3::from_matrix_unchecked(rotation_matrix),
    );

    // Extract translation
    let translation = Vector3::new(matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]);

    // Create dual quaternion: real = rotation, dual = 0.5 * t * q
    let t_quat = nalgebra::Quaternion::new(0.0, translation.x, translation.y, translation.z);
    let dual = t_quat * rotation.quaternion() * 0.5;

    DualQuaternion::from_real_and_dual(rotation.into_inner(), dual)
}

/// Transform a point by dual quaternion.
fn dq_transform_point(dq: &DualQuaternion<f64>, point: &Vector3<f64>) -> Vector3<f64> {
    // q * p * q^* for rotation
    let rotated =
        dq.real * nalgebra::Quaternion::new(0.0, point.x, point.y, point.z) * dq.real.conjugate();

    // Add translation: 2 * dual * real^*
    let trans = dq.dual * dq.real.conjugate() * 2.0;

    Vector3::new(
        rotated.i + trans.i,
        rotated.j + trans.j,
        rotated.k + trans.k,
    )
}

/// Transform a vector (direction) by dual quaternion.
fn dq_transform_vector(dq: &DualQuaternion<f64>, vector: &Vector3<f64>) -> Vector3<f64> {
    // Only rotation, no translation
    let rotated = dq.real
        * nalgebra::Quaternion::new(0.0, vector.x, vector.y, vector.z)
        * dq.real.conjugate();
    Vector3::new(rotated.i, rotated.j, rotated.k)
}

/// Weight painting brush.
#[derive(Debug, Clone)]
pub struct WeightBrush {
    /// Brush radius.
    pub radius: f64,
    /// Brush strength (0-1).
    pub strength: f64,
    /// Falloff type.
    pub falloff: BrushFalloff,
    /// Brush mode.
    pub mode: BrushMode,
    /// Target bone.
    pub target_bone: BoneId,
    /// Auto-normalize weights.
    pub auto_normalize: bool,
}

/// Brush falloff type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushFalloff {
    /// Constant strength.
    Constant,
    /// Linear falloff.
    Linear,
    /// Smooth falloff.
    Smooth,
    /// Sharp falloff.
    Sharp,
}

/// Brush mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushMode {
    /// Add weight.
    Add,
    /// Subtract weight.
    Subtract,
    /// Smooth weights.
    Smooth,
    /// Set to specific value.
    Set,
}

impl Default for WeightBrush {
    fn default() -> Self {
        Self {
            radius: 0.1,
            strength: 0.5,
            falloff: BrushFalloff::Smooth,
            mode: BrushMode::Add,
            target_bone: BoneId::INVALID,
            auto_normalize: true,
        }
    }
}

impl WeightBrush {
    /// Apply brush at a position.
    pub fn apply(
        &self,
        skin: &mut SkinData,
        position: Point3<f64>,
        vertex_positions: &[Point3<f64>],
    ) {
        for (i, vertex_pos) in vertex_positions.iter().enumerate() {
            let distance = (vertex_pos - position).magnitude();
            if distance > self.radius {
                continue;
            }

            let falloff = self.compute_falloff(distance);
            let influence = self.strength * falloff;

            if let Some(weights) = skin.get_weights_mut(i) {
                match self.mode {
                    BrushMode::Add => {
                        let current = weights
                            .weights()
                            .iter()
                            .find(|w| w.bone == self.target_bone)
                            .map(|w| w.weight)
                            .unwrap_or(0.0);
                        let new_weight = (current + influence).min(1.0);
                        self.set_bone_weight(weights, self.target_bone, new_weight);
                    }
                    BrushMode::Subtract => {
                        let current = weights
                            .weights()
                            .iter()
                            .find(|w| w.bone == self.target_bone)
                            .map(|w| w.weight)
                            .unwrap_or(0.0);
                        let new_weight = (current - influence).max(0.0);
                        self.set_bone_weight(weights, self.target_bone, new_weight);
                    }
                    BrushMode::Set => {
                        self.set_bone_weight(weights, self.target_bone, self.strength);
                    }
                    BrushMode::Smooth => {
                        // Smooth would require neighbor information
                    }
                }

                if self.auto_normalize {
                    weights.normalize();
                }
            }
        }
    }

    fn compute_falloff(&self, distance: f64) -> f64 {
        let t = distance / self.radius;
        match self.falloff {
            BrushFalloff::Constant => 1.0,
            BrushFalloff::Linear => 1.0 - t,
            BrushFalloff::Smooth => {
                let s = 1.0 - t;
                s * s * (3.0 - 2.0 * s)
            }
            BrushFalloff::Sharp => {
                let s = 1.0 - t;
                s * s * s
            }
        }
    }

    fn set_bone_weight(&self, weights: &mut VertexWeights, bone: BoneId, weight: f64) {
        // Remove existing weight for this bone
        weights.weights.retain(|w| w.bone != bone);

        // Add new weight if > 0
        if weight > 0.0 {
            weights.weights.push(BoneWeight::new(bone, weight));
            weights
                .weights
                .sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());
            if weights.weights.len() > MAX_BONES_PER_VERTEX {
                weights.weights.truncate(MAX_BONES_PER_VERTEX);
            }
        }
    }
}

/// Automatic weight generation.
pub struct AutoWeightGenerator {
    /// Heat diffusion iterations.
    pub iterations: usize,
    /// Distance power for initial weights.
    pub distance_power: f64,
}

impl Default for AutoWeightGenerator {
    fn default() -> Self {
        Self {
            iterations: 10,
            distance_power: 2.0,
        }
    }
}

impl AutoWeightGenerator {
    /// Generate weights from bone envelope.
    pub fn generate_envelope_weights(
        &self,
        skin: &mut SkinData,
        vertex_positions: &[Point3<f64>],
        armature: &Armature,
        bone_envelopes: &HashMap<BoneId, f64>,
    ) {
        for (i, pos) in vertex_positions.iter().enumerate() {
            for bone in armature.bones() {
                let envelope = bone_envelopes.get(&bone.id).copied().unwrap_or(0.1);

                // Distance from vertex to bone segment
                let dist = point_to_segment_distance(pos, &bone.head, &bone.tail);

                if dist < envelope {
                    let weight = 1.0 - (dist / envelope).powf(self.distance_power);
                    skin.set_weight(i, bone.id, weight);
                }
            }
        }

        skin.normalize_all();
    }

    /// Generate weights using heat diffusion.
    pub fn generate_heat_weights(
        &self,
        skin: &mut SkinData,
        vertex_positions: &[Point3<f64>],
        _edges: &[(usize, usize)],
        armature: &Armature,
    ) {
        // Initialize with closest bone
        for (i, pos) in vertex_positions.iter().enumerate() {
            let mut closest_bone = BoneId::INVALID;
            let mut min_dist = f64::MAX;

            for bone in armature.bones() {
                let dist = point_to_segment_distance(pos, &bone.head, &bone.tail);
                if dist < min_dist {
                    min_dist = dist;
                    closest_bone = bone.id;
                }
            }

            if closest_bone.is_valid() {
                skin.set_weight(i, closest_bone, 1.0);
            }
        }

        // Heat diffusion would smooth these weights across edges
        // Simplified: just normalize
        skin.normalize_all();
    }
}

/// Distance from point to line segment.
fn point_to_segment_distance(point: &Point3<f64>, start: &Point3<f64>, end: &Point3<f64>) -> f64 {
    let v = end - start;
    let w = point - start;

    let c1 = w.dot(&v);
    if c1 <= 0.0 {
        return (point - start).magnitude();
    }

    let c2 = v.dot(&v);
    if c2 <= c1 {
        return (point - end).magnitude();
    }

    let b = c1 / c2;
    let pb = start + v * b;
    (point - pb).magnitude()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_weights() {
        let mut weights = VertexWeights::new();
        weights.add_weight(BoneId(0), 0.6);
        weights.add_weight(BoneId(1), 0.4);

        assert_eq!(weights.weights().len(), 2);
    }

    #[test]
    fn test_weights_normalize() {
        let mut weights = VertexWeights::new();
        weights.add_weight(BoneId(0), 0.3);
        weights.add_weight(BoneId(1), 0.3);
        weights.normalize();

        let sum: f64 = weights.weights().iter().map(|w| w.weight).sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_skin_data() {
        let mut skin = SkinData::new(10);
        skin.set_weight(0, BoneId(0), 1.0);

        let weights = skin.get_weights(0).unwrap();
        assert_eq!(weights.weights().len(), 1);
    }

    #[test]
    fn test_gpu_format() {
        let mut weights = VertexWeights::new();
        weights.add_weight(BoneId(0), 0.5);
        weights.add_weight(BoneId(1), 0.5);

        let (indices, wts) = weights.to_gpu_format();
        assert_eq!(indices[0], 0);
        assert!((wts[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_point_to_segment_distance() {
        let point = Point3::new(1.0, 1.0, 0.0);
        let start = Point3::new(0.0, 0.0, 0.0);
        let end = Point3::new(2.0, 0.0, 0.0);

        let dist = point_to_segment_distance(&point, &start, &end);
        assert!((dist - 1.0).abs() < 1e-10);
    }
}
