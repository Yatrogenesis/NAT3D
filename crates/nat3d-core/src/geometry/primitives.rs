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

//! Primitive geometry generation.
//!
//! This module provides functions to create standard 3D primitives
//! like cubes, spheres, cylinders, cones, tori, and planes.

use super::{Mesh, Normal, Position, TexCoord, VertexData};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Types of primitive shapes that can be generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Primitive {
    /// Cube/box primitive.
    Cube,
    /// Sphere primitive.
    Sphere,
    /// Cylinder primitive.
    Cylinder,
    /// Cone primitive.
    Cone,
    /// Torus (donut) primitive.
    Torus,
    /// Plane/quad primitive.
    Plane,
    /// Capsule primitive.
    Capsule,
    /// Pyramid primitive.
    Pyramid,
    /// Tetrahedron primitive.
    Tetrahedron,
    /// Icosahedron primitive.
    Icosahedron,
}

/// Parameters for primitive generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimitiveParams {
    /// Name for the generated mesh.
    pub name: String,
    /// Size along X axis (width).
    pub width: f64,
    /// Size along Y axis (height).
    pub height: f64,
    /// Size along Z axis (depth).
    pub depth: f64,
    /// Radius for curved primitives.
    pub radius: f64,
    /// Secondary radius (for torus inner radius, etc.).
    pub radius2: f64,
    /// Number of segments around circumference.
    pub segments: u32,
    /// Number of rings/stacks.
    pub rings: u32,
    /// Whether to generate UVs.
    pub generate_uvs: bool,
    /// Whether to generate normals.
    pub generate_normals: bool,
    /// Whether to smooth normals.
    pub smooth_normals: bool,
}

impl Default for PrimitiveParams {
    fn default() -> Self {
        Self {
            name: "Primitive".into(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
            radius: 0.5,
            radius2: 0.25,
            segments: 32,
            rings: 16,
            generate_uvs: true,
            generate_normals: true,
            smooth_normals: true,
        }
    }
}

impl PrimitiveParams {
    /// Create params for a cube with given size.
    #[must_use]
    pub fn cube(size: f64) -> Self {
        Self {
            name: "Cube".into(),
            width: size,
            height: size,
            depth: size,
            ..Default::default()
        }
    }

    /// Create params for a box with given dimensions.
    #[must_use]
    pub fn box_shape(width: f64, height: f64, depth: f64) -> Self {
        Self {
            name: "Box".into(),
            width,
            height,
            depth,
            ..Default::default()
        }
    }

    /// Create params for a sphere with given radius.
    #[must_use]
    pub fn sphere(radius: f64) -> Self {
        Self {
            name: "Sphere".into(),
            radius,
            ..Default::default()
        }
    }

    /// Create params for a cylinder with given radius and height.
    #[must_use]
    pub fn cylinder(radius: f64, height: f64) -> Self {
        Self {
            name: "Cylinder".into(),
            radius,
            height,
            ..Default::default()
        }
    }

    /// Create params for a cone with given radius and height.
    #[must_use]
    pub fn cone(radius: f64, height: f64) -> Self {
        Self {
            name: "Cone".into(),
            radius,
            height,
            ..Default::default()
        }
    }

    /// Create params for a torus with given radii.
    #[must_use]
    pub fn torus(major_radius: f64, minor_radius: f64) -> Self {
        Self {
            name: "Torus".into(),
            radius: major_radius,
            radius2: minor_radius,
            ..Default::default()
        }
    }

    /// Create params for a plane with given dimensions.
    #[must_use]
    pub fn plane(width: f64, depth: f64) -> Self {
        Self {
            name: "Plane".into(),
            width,
            depth,
            ..Default::default()
        }
    }

    /// Builder method to set segment count.
    #[must_use]
    pub fn with_segments(mut self, segments: u32) -> Self {
        self.segments = segments.max(3);
        self
    }

    /// Builder method to set ring count.
    #[must_use]
    pub fn with_rings(mut self, rings: u32) -> Self {
        self.rings = rings.max(1);
        self
    }

    /// Builder method to set name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// Generate a primitive mesh.
#[must_use]
pub fn create_primitive(primitive: Primitive, params: &PrimitiveParams) -> Mesh {
    match primitive {
        Primitive::Cube => create_cube(params),
        Primitive::Sphere => create_sphere(params),
        Primitive::Cylinder => create_cylinder(params),
        Primitive::Cone => create_cone(params),
        Primitive::Torus => create_torus(params),
        Primitive::Plane => create_plane(params),
        Primitive::Capsule => create_capsule(params),
        Primitive::Pyramid => create_pyramid(params),
        Primitive::Tetrahedron => create_tetrahedron(params),
        Primitive::Icosahedron => create_icosahedron(params),
    }
}

/// Create a cube/box mesh.
#[must_use]
pub fn create_cube(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let hw = params.width / 2.0;
    let hh = params.height / 2.0;
    let hd = params.depth / 2.0;

    // Define the 8 corners
    let corners = [
        Position::new(-hw, -hh, -hd), // 0: back-bottom-left
        Position::new(hw, -hh, -hd),  // 1: back-bottom-right
        Position::new(hw, hh, -hd),   // 2: back-top-right
        Position::new(-hw, hh, -hd),  // 3: back-top-left
        Position::new(-hw, -hh, hd),  // 4: front-bottom-left
        Position::new(hw, -hh, hd),   // 5: front-bottom-right
        Position::new(hw, hh, hd),    // 6: front-top-right
        Position::new(-hw, hh, hd),   // 7: front-top-left
    ];

    // Face definitions with normals and UVs
    // Each face has 4 vertices with their own normals and UVs
    let faces = [
        // Front face (Z+)
        ([4, 5, 6, 7], Normal::new(0.0, 0.0, 1.0)),
        // Back face (Z-)
        ([1, 0, 3, 2], Normal::new(0.0, 0.0, -1.0)),
        // Top face (Y+)
        ([7, 6, 2, 3], Normal::new(0.0, 1.0, 0.0)),
        // Bottom face (Y-)
        ([0, 1, 5, 4], Normal::new(0.0, -1.0, 0.0)),
        // Right face (X+)
        ([5, 1, 2, 6], Normal::new(1.0, 0.0, 0.0)),
        // Left face (X-)
        ([0, 4, 7, 3], Normal::new(-1.0, 0.0, 0.0)),
    ];

    let uvs = [
        TexCoord::new(0.0, 0.0),
        TexCoord::new(1.0, 0.0),
        TexCoord::new(1.0, 1.0),
        TexCoord::new(0.0, 1.0),
    ];

    for (corner_indices, normal) in &faces {
        let base = mesh.vertex_count();

        for (i, &ci) in corner_indices.iter().enumerate() {
            let mut data = VertexData::from_position(corners[ci]);
            if params.generate_normals {
                data.normal = Some(*normal);
            }
            if params.generate_uvs {
                data.uv = Some(uvs[i]);
            }
            mesh.add_vertex(data);
        }

        let _ = mesh.add_quad(base, base + 1, base + 2, base + 3);
    }

    mesh
}

/// Create a UV sphere mesh.
#[must_use]
pub fn create_sphere(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let segments = params.segments.max(4) as usize;
    let rings = params.rings.max(2) as usize;
    let radius = params.radius;

    // Generate vertices
    for ring in 0..=rings {
        let v = ring as f64 / rings as f64;
        let phi = v * PI;

        for seg in 0..=segments {
            let u = seg as f64 / segments as f64;
            let theta = u * 2.0 * PI;

            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.cos();
            let z = radius * phi.sin() * theta.sin();

            let pos = Position::new(x, y, z);
            let normal = Normal::new(x, y, z).normalize();
            let uv = TexCoord::new(u, 1.0 - v);

            let mut data = VertexData::from_position(pos);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            if params.generate_uvs {
                data.uv = Some(uv);
            }
            mesh.add_vertex(data);
        }
    }

    // Generate faces
    let stride = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let i0 = ring * stride + seg;
            let i1 = i0 + 1;
            let i2 = i0 + stride + 1;
            let i3 = i0 + stride;

            // Skip degenerate triangles at poles
            if ring > 0 {
                let _ = mesh.add_triangle(i0, i1, i2);
            }
            if ring < rings - 1 {
                let _ = mesh.add_triangle(i0, i2, i3);
            }
        }
    }

    mesh
}

/// Create a cylinder mesh.
#[must_use]
pub fn create_cylinder(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let segments = params.segments.max(3) as usize;
    let radius = params.radius;
    let half_height = params.height / 2.0;

    // Bottom center vertex
    let bottom_center = mesh.add_vertex({
        let mut data = VertexData::from_position(Position::new(0.0, -half_height, 0.0));
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, -1.0, 0.0));
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(0.5, 0.5));
        }
        data
    });

    // Top center vertex
    let top_center = mesh.add_vertex({
        let mut data = VertexData::from_position(Position::new(0.0, half_height, 0.0));
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, 1.0, 0.0));
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(0.5, 0.5));
        }
        data
    });

    // Generate rim vertices for bottom cap (with down normal)
    let bottom_rim_start = mesh.vertex_count();
    for i in 0..=segments {
        let angle = (i as f64 / segments as f64) * 2.0 * PI;
        let x = radius * angle.cos();
        let z = radius * angle.sin();

        let mut data = VertexData::from_position(Position::new(x, -half_height, z));
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, -1.0, 0.0));
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(
                0.5 + 0.5 * angle.cos(),
                0.5 + 0.5 * angle.sin(),
            ));
        }
        mesh.add_vertex(data);
    }

    // Generate rim vertices for top cap (with up normal)
    let top_rim_start = mesh.vertex_count();
    for i in 0..=segments {
        let angle = (i as f64 / segments as f64) * 2.0 * PI;
        let x = radius * angle.cos();
        let z = radius * angle.sin();

        let mut data = VertexData::from_position(Position::new(x, half_height, z));
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, 1.0, 0.0));
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(
                0.5 + 0.5 * angle.cos(),
                0.5 + 0.5 * angle.sin(),
            ));
        }
        mesh.add_vertex(data);
    }

    // Generate rim vertices for sides (with outward normal)
    let side_bottom_start = mesh.vertex_count();
    for i in 0..=segments {
        let angle = (i as f64 / segments as f64) * 2.0 * PI;
        let x = angle.cos();
        let z = angle.sin();

        // Bottom side vertex
        let mut data =
            VertexData::from_position(Position::new(radius * x, -half_height, radius * z));
        if params.generate_normals {
            data.normal = Some(Normal::new(x, 0.0, z));
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(i as f64 / segments as f64, 0.0));
        }
        mesh.add_vertex(data);
    }

    let side_top_start = mesh.vertex_count();
    for i in 0..=segments {
        let angle = (i as f64 / segments as f64) * 2.0 * PI;
        let x = angle.cos();
        let z = angle.sin();

        // Top side vertex
        let mut data =
            VertexData::from_position(Position::new(radius * x, half_height, radius * z));
        if params.generate_normals {
            data.normal = Some(Normal::new(x, 0.0, z));
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(i as f64 / segments as f64, 1.0));
        }
        mesh.add_vertex(data);
    }

    // Generate bottom cap faces
    for i in 0..segments {
        let _ = mesh.add_triangle(
            bottom_center,
            bottom_rim_start + i + 1,
            bottom_rim_start + i,
        );
    }

    // Generate top cap faces
    for i in 0..segments {
        let _ = mesh.add_triangle(top_center, top_rim_start + i, top_rim_start + i + 1);
    }

    // Generate side faces
    for i in 0..segments {
        let b0 = side_bottom_start + i;
        let b1 = side_bottom_start + i + 1;
        let t0 = side_top_start + i;
        let t1 = side_top_start + i + 1;

        let _ = mesh.add_quad(b0, b1, t1, t0);
    }

    mesh
}

/// Create a cone mesh.
#[must_use]
pub fn create_cone(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let segments = params.segments.max(3) as usize;
    let radius = params.radius;
    let half_height = params.height / 2.0;

    // Bottom center
    let bottom_center = mesh.add_vertex({
        let mut data = VertexData::from_position(Position::new(0.0, -half_height, 0.0));
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, -1.0, 0.0));
        }
        data
    });

    // Apex
    let apex = mesh.add_vertex({
        // Normal will be averaged from side faces
        VertexData::from_position(Position::new(0.0, half_height, 0.0))
    });

    // Bottom rim vertices (for cap)
    let cap_start = mesh.vertex_count();
    for i in 0..=segments {
        let angle = (i as f64 / segments as f64) * 2.0 * PI;
        let x = radius * angle.cos();
        let z = radius * angle.sin();

        let mut data = VertexData::from_position(Position::new(x, -half_height, z));
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, -1.0, 0.0));
        }
        mesh.add_vertex(data);
    }

    // Side vertices (with proper normals)
    let side_start = mesh.vertex_count();
    let slope = radius / params.height;
    let ny = 1.0 / (1.0 + slope * slope).sqrt();
    let nxz = slope * ny;

    for i in 0..=segments {
        let angle = (i as f64 / segments as f64) * 2.0 * PI;
        let x = angle.cos();
        let z = angle.sin();

        let mut data =
            VertexData::from_position(Position::new(radius * x, -half_height, radius * z));
        if params.generate_normals {
            data.normal = Some(Normal::new(nxz * x, ny, nxz * z).normalize());
        }
        if params.generate_uvs {
            data.uv = Some(TexCoord::new(i as f64 / segments as f64, 0.0));
        }
        mesh.add_vertex(data);
    }

    // Generate bottom cap
    for i in 0..segments {
        let _ = mesh.add_triangle(bottom_center, cap_start + i + 1, cap_start + i);
    }

    // Generate side faces
    for i in 0..segments {
        let _ = mesh.add_triangle(side_start + i, side_start + i + 1, apex);
    }

    mesh
}

/// Create a torus mesh.
#[must_use]
pub fn create_torus(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let segments = params.segments.max(3) as usize;
    let rings = params.rings.max(3) as usize;
    let major_radius = params.radius;
    let minor_radius = params.radius2;

    // Generate vertices
    for ring in 0..=rings {
        let u = ring as f64 / rings as f64;
        let phi = u * 2.0 * PI;
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();

        for seg in 0..=segments {
            let v = seg as f64 / segments as f64;
            let theta = v * 2.0 * PI;
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            let x = (major_radius + minor_radius * cos_theta) * cos_phi;
            let y = minor_radius * sin_theta;
            let z = (major_radius + minor_radius * cos_theta) * sin_phi;

            let nx = cos_theta * cos_phi;
            let ny = sin_theta;
            let nz = cos_theta * sin_phi;

            let pos = Position::new(x, y, z);
            let normal = Normal::new(nx, ny, nz).normalize();
            let uv = TexCoord::new(u, v);

            let mut data = VertexData::from_position(pos);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            if params.generate_uvs {
                data.uv = Some(uv);
            }
            mesh.add_vertex(data);
        }
    }

    // Generate faces
    let stride = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let i0 = ring * stride + seg;
            let i1 = i0 + 1;
            let i2 = i0 + stride + 1;
            let i3 = i0 + stride;

            let _ = mesh.add_quad(i0, i1, i2, i3);
        }
    }

    mesh
}

/// Create a plane mesh.
#[must_use]
pub fn create_plane(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let hw = params.width / 2.0;
    let hd = params.depth / 2.0;

    let positions = [
        Position::new(-hw, 0.0, -hd),
        Position::new(hw, 0.0, -hd),
        Position::new(hw, 0.0, hd),
        Position::new(-hw, 0.0, hd),
    ];

    let uvs = [
        TexCoord::new(0.0, 0.0),
        TexCoord::new(1.0, 0.0),
        TexCoord::new(1.0, 1.0),
        TexCoord::new(0.0, 1.0),
    ];

    let normal = Normal::new(0.0, 1.0, 0.0);

    for (i, pos) in positions.iter().enumerate() {
        let mut data = VertexData::from_position(*pos);
        if params.generate_normals {
            data.normal = Some(normal);
        }
        if params.generate_uvs {
            data.uv = Some(uvs[i]);
        }
        mesh.add_vertex(data);
    }

    let _ = mesh.add_quad(0, 1, 2, 3);

    mesh
}

/// Create a capsule mesh.
#[must_use]
pub fn create_capsule(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let segments = params.segments.max(4) as usize;
    let rings = (params.rings.max(4) / 2) as usize; // Half for each hemisphere
    let radius = params.radius;
    let half_height = (params.height / 2.0) - radius;

    // Generate top hemisphere
    for ring in 0..=rings {
        let v = ring as f64 / (rings * 2) as f64;
        let phi = v * PI;

        for seg in 0..=segments {
            let u = seg as f64 / segments as f64;
            let theta = u * 2.0 * PI;

            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.cos() + half_height;
            let z = radius * phi.sin() * theta.sin();

            let nx = phi.sin() * theta.cos();
            let ny = phi.cos();
            let nz = phi.sin() * theta.sin();

            let mut data = VertexData::from_position(Position::new(x, y, z));
            if params.generate_normals {
                data.normal = Some(Normal::new(nx, ny, nz).normalize());
            }
            if params.generate_uvs {
                data.uv = Some(TexCoord::new(u, v * 0.25));
            }
            mesh.add_vertex(data);
        }
    }

    // Generate cylinder middle
    let cyl_start = mesh.vertex_count();
    for i in 0..=1 {
        let y = if i == 0 { half_height } else { -half_height };
        let v_base = if i == 0 { 0.25 } else { 0.75 };

        for seg in 0..=segments {
            let u = seg as f64 / segments as f64;
            let theta = u * 2.0 * PI;

            let x = radius * theta.cos();
            let z = radius * theta.sin();

            let mut data = VertexData::from_position(Position::new(x, y, z));
            if params.generate_normals {
                data.normal = Some(Normal::new(theta.cos(), 0.0, theta.sin()));
            }
            if params.generate_uvs {
                data.uv = Some(TexCoord::new(u, v_base));
            }
            mesh.add_vertex(data);
        }
    }

    // Generate bottom hemisphere
    let bottom_start = mesh.vertex_count();
    for ring in 0..=rings {
        let v = 0.5 + ring as f64 / (rings * 2) as f64;
        let phi = v * PI;

        for seg in 0..=segments {
            let u = seg as f64 / segments as f64;
            let theta = u * 2.0 * PI;

            let x = radius * phi.sin() * theta.cos();
            let y = radius * phi.cos() - half_height;
            let z = radius * phi.sin() * theta.sin();

            let nx = phi.sin() * theta.cos();
            let ny = phi.cos();
            let nz = phi.sin() * theta.sin();

            let mut data = VertexData::from_position(Position::new(x, y, z));
            if params.generate_normals {
                data.normal = Some(Normal::new(nx, ny, nz).normalize());
            }
            if params.generate_uvs {
                data.uv = Some(TexCoord::new(u, 0.75 + v * 0.25));
            }
            mesh.add_vertex(data);
        }
    }

    let stride = segments + 1;

    // Top hemisphere faces
    for ring in 0..rings {
        for seg in 0..segments {
            let i0 = ring * stride + seg;
            let i1 = i0 + 1;
            let i2 = i0 + stride + 1;
            let i3 = i0 + stride;

            if ring > 0 {
                let _ = mesh.add_triangle(i0, i1, i2);
            }
            let _ = mesh.add_triangle(i0, i2, i3);
        }
    }

    // Cylinder faces
    for seg in 0..segments {
        let b0 = cyl_start + seg;
        let b1 = cyl_start + seg + 1;
        let t0 = cyl_start + stride + seg;
        let t1 = cyl_start + stride + seg + 1;

        let _ = mesh.add_quad(b0, b1, t1, t0);
    }

    // Bottom hemisphere faces
    for ring in 0..rings {
        for seg in 0..segments {
            let i0 = bottom_start + ring * stride + seg;
            let i1 = i0 + 1;
            let i2 = i0 + stride + 1;
            let i3 = i0 + stride;

            let _ = mesh.add_triangle(i0, i1, i2);
            if ring < rings - 1 {
                let _ = mesh.add_triangle(i0, i2, i3);
            }
        }
    }

    mesh
}

/// Create a pyramid mesh.
#[must_use]
pub fn create_pyramid(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let hw = params.width / 2.0;
    let hh = params.height / 2.0;
    let hd = params.depth / 2.0;

    // Base vertices
    let base = [
        Position::new(-hw, -hh, -hd),
        Position::new(hw, -hh, -hd),
        Position::new(hw, -hh, hd),
        Position::new(-hw, -hh, hd),
    ];

    // Apex
    let apex = Position::new(0.0, hh, 0.0);

    // Add base vertices with down normal
    let base_start = mesh.vertex_count();
    for pos in &base {
        let mut data = VertexData::from_position(*pos);
        if params.generate_normals {
            data.normal = Some(Normal::new(0.0, -1.0, 0.0));
        }
        mesh.add_vertex(data);
    }

    // Add base face
    let _ = mesh.add_quad(base_start, base_start + 3, base_start + 2, base_start + 1);

    // Add side faces with proper normals
    let sides = [(0, 1), (1, 2), (2, 3), (3, 0)];
    for (i0, i1) in sides {
        let p0 = base[i0];
        let p1 = base[i1];

        // Compute face normal
        let v1 = p1 - p0;
        let v2 = apex - p0;
        let normal = v1.cross(&v2).normalize();

        let vi0 = mesh.add_vertex({
            let mut data = VertexData::from_position(p0);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            data
        });

        let vi1 = mesh.add_vertex({
            let mut data = VertexData::from_position(p1);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            data
        });

        let vi_apex = mesh.add_vertex({
            let mut data = VertexData::from_position(apex);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            data
        });

        let _ = mesh.add_triangle(vi0, vi1, vi_apex);
    }

    mesh
}

/// Create a tetrahedron mesh.
#[must_use]
pub fn create_tetrahedron(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let size = params.width;
    let h = size * (2.0_f64 / 3.0).sqrt();
    let r = size / 3.0_f64.sqrt();

    // Tetrahedron vertices
    let vertices = [
        Position::new(0.0, h / 2.0, -r),
        Position::new(-size / 2.0, -h / 2.0, r / 2.0),
        Position::new(size / 2.0, -h / 2.0, r / 2.0),
        Position::new(0.0, -h / 2.0, -r),
    ];

    // Faces (with proper winding)
    let faces = [(0, 1, 2), (0, 2, 3), (0, 3, 1), (1, 3, 2)];

    for (i0, i1, i2) in faces {
        let p0 = vertices[i0];
        let p1 = vertices[i1];
        let p2 = vertices[i2];

        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let normal = v1.cross(&v2).normalize();

        let vi0 = mesh.add_vertex({
            let mut data = VertexData::from_position(p0);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            data
        });

        let vi1 = mesh.add_vertex({
            let mut data = VertexData::from_position(p1);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            data
        });

        let vi2 = mesh.add_vertex({
            let mut data = VertexData::from_position(p2);
            if params.generate_normals {
                data.normal = Some(normal);
            }
            data
        });

        let _ = mesh.add_triangle(vi0, vi1, vi2);
    }

    mesh
}

/// Create an icosahedron mesh.
#[must_use]
pub fn create_icosahedron(params: &PrimitiveParams) -> Mesh {
    let mut mesh = Mesh::new(&params.name);

    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let scale = params.radius / (1.0 + phi * phi).sqrt();

    // 12 vertices of an icosahedron
    let vertices = [
        Position::new(-1.0, phi, 0.0) * scale,
        Position::new(1.0, phi, 0.0) * scale,
        Position::new(-1.0, -phi, 0.0) * scale,
        Position::new(1.0, -phi, 0.0) * scale,
        Position::new(0.0, -1.0, phi) * scale,
        Position::new(0.0, 1.0, phi) * scale,
        Position::new(0.0, -1.0, -phi) * scale,
        Position::new(0.0, 1.0, -phi) * scale,
        Position::new(phi, 0.0, -1.0) * scale,
        Position::new(phi, 0.0, 1.0) * scale,
        Position::new(-phi, 0.0, -1.0) * scale,
        Position::new(-phi, 0.0, 1.0) * scale,
    ];

    // 20 triangular faces
    let faces = [
        (0, 11, 5),
        (0, 5, 1),
        (0, 1, 7),
        (0, 7, 10),
        (0, 10, 11),
        (1, 5, 9),
        (5, 11, 4),
        (11, 10, 2),
        (10, 7, 6),
        (7, 1, 8),
        (3, 9, 4),
        (3, 4, 2),
        (3, 2, 6),
        (3, 6, 8),
        (3, 8, 9),
        (4, 9, 5),
        (2, 4, 11),
        (6, 2, 10),
        (8, 6, 7),
        (9, 8, 1),
    ];

    // Add vertices
    for pos in &vertices {
        let normal = pos.coords.normalize();
        let mut data = VertexData::from_position(*pos);
        if params.generate_normals {
            data.normal = Some(normal);
        }
        mesh.add_vertex(data);
    }

    // Add faces
    for (i0, i1, i2) in faces {
        let _ = mesh.add_triangle(i0, i1, i2);
    }

    if params.smooth_normals {
        mesh.compute_vertex_normals();
    }

    mesh
}

// ══════════════════════════════════════════════════════════════════════════════
// Convenience methods on Mesh
// ══════════════════════════════════════════════════════════════════════════════

impl Mesh {
    /// Create a cube mesh with the given size.
    #[must_use]
    pub fn cube(size: f64) -> Self {
        create_cube(&PrimitiveParams::cube(size))
    }

    /// Create a sphere mesh with the given radius.
    #[must_use]
    pub fn sphere(radius: f64) -> Self {
        create_sphere(&PrimitiveParams::sphere(radius))
    }

    /// Create a cylinder mesh with the given radius and height.
    #[must_use]
    pub fn cylinder(radius: f64, height: f64) -> Self {
        create_cylinder(&PrimitiveParams::cylinder(radius, height))
    }

    /// Create a plane mesh with the given dimensions.
    #[must_use]
    pub fn plane(width: f64, depth: f64) -> Self {
        create_plane(&PrimitiveParams::plane(width, depth))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cube_creation() {
        let cube = Mesh::cube(2.0);
        assert_eq!(cube.face_count(), 6);
        assert_eq!(cube.vertex_count(), 24); // 4 vertices per face * 6 faces
    }

    #[test]
    fn test_sphere_creation() {
        let params = PrimitiveParams::sphere(1.0).with_segments(8).with_rings(4);
        let sphere = create_sphere(&params);
        assert!(sphere.vertex_count() > 0);
        assert!(sphere.face_count() > 0);
    }

    #[test]
    fn test_cylinder_creation() {
        let params = PrimitiveParams::cylinder(0.5, 2.0).with_segments(8);
        let cylinder = create_cylinder(&params);
        assert!(cylinder.vertex_count() > 0);
        assert!(cylinder.face_count() > 0);
    }

    #[test]
    fn test_plane_creation() {
        let plane = Mesh::plane(2.0, 2.0);
        assert_eq!(plane.vertex_count(), 4);
        assert_eq!(plane.face_count(), 1);
    }

    #[test]
    fn test_torus_creation() {
        let params = PrimitiveParams::torus(1.0, 0.3)
            .with_segments(16)
            .with_rings(8);
        let torus = create_torus(&params);
        assert!(torus.vertex_count() > 0);
        assert!(torus.face_count() > 0);
    }

    #[test]
    fn test_icosahedron_creation() {
        let params = PrimitiveParams::default();
        let ico = create_icosahedron(&params);
        assert_eq!(ico.vertex_count(), 12);
        assert_eq!(ico.face_count(), 20);
    }
}
