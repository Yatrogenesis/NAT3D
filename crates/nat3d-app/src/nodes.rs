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

//! Node-based editor system for materials, compositing, and geometry.

use std::collections::HashMap;

/// Unique node ID.
#[allow(dead_code)]
pub type NodeId = u64;

/// Unique socket ID.
#[allow(dead_code)]
pub type SocketId = u64;

/// Socket direction.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDirection {
    Input,
    Output,
}

/// Socket data type.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Float,
    Vector2,
    Vector3,
    Vector4,
    Color,
    Shader,
    Geometry,
    Image,
    Bool,
    Int,
    String,
}

#[allow(dead_code)]
impl SocketType {
    /// Get color for socket type.
    pub fn color(&self) -> [u8; 3] {
        match self {
            Self::Float => [160, 160, 160],
            Self::Vector2 => [100, 100, 200],
            Self::Vector3 => [100, 100, 230],
            Self::Vector4 => [130, 130, 255],
            Self::Color => [230, 230, 100],
            Self::Shader => [100, 230, 100],
            Self::Geometry => [100, 200, 200],
            Self::Image => [200, 100, 200],
            Self::Bool => [200, 100, 100],
            Self::Int => [130, 200, 130],
            Self::String => [200, 200, 200],
        }
    }

    /// Check if types are compatible for connection.
    pub fn is_compatible(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        // Allow some implicit conversions
        matches!(
            (self, other),
            (Self::Float, Self::Int)
                | (Self::Int, Self::Float)
                | (Self::Vector3, Self::Color)
                | (Self::Color, Self::Vector3)
                | (Self::Vector4, Self::Color)
                | (Self::Color, Self::Vector4)
        )
    }
}

/// Node socket definition.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeSocket {
    pub id: SocketId,
    pub name: String,
    pub socket_type: SocketType,
    pub direction: SocketDirection,
    pub default_value: SocketValue,
}

/// Socket value.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub enum SocketValue {
    Float(f32),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    Color([f32; 4]),
    Bool(bool),
    Int(i32),
    String(String),
    #[default]
    None,
}

#[allow(dead_code)]
impl SocketValue {
    /// Create default value for socket type.
    pub fn default_for_type(socket_type: SocketType) -> Self {
        match socket_type {
            SocketType::Float => Self::Float(0.0),
            SocketType::Vector2 => Self::Vector2([0.0, 0.0]),
            SocketType::Vector3 => Self::Vector3([0.0, 0.0, 0.0]),
            SocketType::Vector4 => Self::Vector4([0.0, 0.0, 0.0, 1.0]),
            SocketType::Color => Self::Color([0.8, 0.8, 0.8, 1.0]),
            SocketType::Bool => Self::Bool(false),
            SocketType::Int => Self::Int(0),
            SocketType::String => Self::String(String::new()),
            _ => Self::None,
        }
    }
}

/// Node category.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeCategory {
    Input,
    Output,
    Shader,
    Texture,
    Color,
    Vector,
    Converter,
    Math,
    Geometry,
    Layout,
}

/// Node definition.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub category: NodeCategory,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub inputs: Vec<NodeSocket>,
    pub outputs: Vec<NodeSocket>,
    pub collapsed: bool,
    pub color: Option<[u8; 3]>,
}

#[allow(dead_code)]
impl Node {
    /// Create a new node.
    pub fn new(id: NodeId, name: &str, category: NodeCategory) -> Self {
        Self {
            id,
            name: name.to_string(),
            category,
            position: [0.0, 0.0],
            size: [200.0, 100.0],
            inputs: Vec::new(),
            outputs: Vec::new(),
            collapsed: false,
            color: None,
        }
    }

    /// Add an input socket.
    pub fn add_input(&mut self, id: SocketId, name: &str, socket_type: SocketType) {
        self.inputs.push(NodeSocket {
            id,
            name: name.to_string(),
            socket_type,
            direction: SocketDirection::Input,
            default_value: SocketValue::default_for_type(socket_type),
        });
    }

    /// Add an output socket.
    pub fn add_output(&mut self, id: SocketId, name: &str, socket_type: SocketType) {
        self.outputs.push(NodeSocket {
            id,
            name: name.to_string(),
            socket_type,
            direction: SocketDirection::Output,
            default_value: SocketValue::None,
        });
    }

    /// Get socket by ID.
    pub fn get_socket(&self, id: SocketId) -> Option<&NodeSocket> {
        self.inputs
            .iter()
            .find(|s| s.id == id)
            .or_else(|| self.outputs.iter().find(|s| s.id == id))
    }

    /// Get socket position in screen space.
    pub fn socket_position(&self, socket_id: SocketId) -> Option<[f32; 2]> {
        let socket_height = 25.0;
        let header_height = 30.0;

        for (i, socket) in self.inputs.iter().enumerate() {
            if socket.id == socket_id {
                return Some([
                    self.position[0],
                    self.position[1] + header_height + socket_height * (i as f32 + 0.5),
                ]);
            }
        }

        for (i, socket) in self.outputs.iter().enumerate() {
            if socket.id == socket_id {
                return Some([
                    self.position[0] + self.size[0],
                    self.position[1] + header_height + socket_height * (i as f32 + 0.5),
                ]);
            }
        }

        None
    }
}

/// Connection between nodes.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeConnection {
    pub from_node: NodeId,
    pub from_socket: SocketId,
    pub to_node: NodeId,
    pub to_socket: SocketId,
}

/// Node graph for material/compositing.
#[allow(dead_code)]
pub struct NodeGraph {
    nodes: HashMap<NodeId, Node>,
    connections: Vec<NodeConnection>,
    next_node_id: NodeId,
    next_socket_id: SocketId,
    pub name: String,
    pub offset: [f32; 2],
    pub zoom: f32,
    selected_nodes: Vec<NodeId>,
}

#[allow(dead_code)]
impl NodeGraph {
    /// Create a new node graph.
    pub fn new(name: &str) -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            next_node_id: 1,
            next_socket_id: 1,
            name: name.to_string(),
            offset: [0.0, 0.0],
            zoom: 1.0,
            selected_nodes: Vec::new(),
        }
    }

    /// Create a default material graph.
    pub fn default_material() -> Self {
        let mut graph = Self::new("Material");

        // Add output node
        let output_id = graph.add_node("Material Output", NodeCategory::Output);
        // Pre-allocate socket IDs
        let sid1 = graph.next_socket_id();
        let sid2 = graph.next_socket_id();
        let sid3 = graph.next_socket_id();
        if let Some(node) = graph.get_node_mut(output_id) {
            node.position = [400.0, 100.0];
            node.add_input(sid1, "Surface", SocketType::Shader);
            node.add_input(sid2, "Volume", SocketType::Shader);
            node.add_input(sid3, "Displacement", SocketType::Vector3);
        }

        // Add principled BSDF
        let bsdf_id = graph.add_node("Principled BSDF", NodeCategory::Shader);
        // Pre-allocate socket IDs
        let sid1 = graph.next_socket_id();
        let sid2 = graph.next_socket_id();
        let sid3 = graph.next_socket_id();
        let sid4 = graph.next_socket_id();
        let sid5 = graph.next_socket_id();
        if let Some(node) = graph.get_node_mut(bsdf_id) {
            node.position = [100.0, 50.0];
            node.add_input(sid1, "Base Color", SocketType::Color);
            node.add_input(sid2, "Metallic", SocketType::Float);
            node.add_input(sid3, "Roughness", SocketType::Float);
            node.add_input(sid4, "Normal", SocketType::Vector3);
            node.add_output(sid5, "BSDF", SocketType::Shader);
        }

        graph
    }

    fn next_socket_id(&mut self) -> SocketId {
        let id = self.next_socket_id;
        self.next_socket_id += 1;
        id
    }

    /// Allocate a fresh socket ID (public alias for external node construction).
    pub fn alloc_socket_id(&mut self) -> SocketId {
        self.next_socket_id()
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, name: &str, category: NodeCategory) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        let node = Node::new(id, name, category);
        self.nodes.insert(id, node);
        id
    }

    /// Remove a node.
    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.remove(&id);
        self.connections
            .retain(|c| c.from_node != id && c.to_node != id);
        self.selected_nodes.retain(|&n| n != id);
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    /// Get a node mutably.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// Get all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Get all connections.
    pub fn connections(&self) -> &[NodeConnection] {
        &self.connections
    }

    /// Connect two sockets.
    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_socket: SocketId,
        to_node: NodeId,
        to_socket: SocketId,
    ) -> bool {
        // Verify connection is valid
        let from_type = self
            .nodes
            .get(&from_node)
            .and_then(|n| n.outputs.iter().find(|s| s.id == from_socket))
            .map(|s| s.socket_type);
        let to_type = self
            .nodes
            .get(&to_node)
            .and_then(|n| n.inputs.iter().find(|s| s.id == to_socket))
            .map(|s| s.socket_type);

        if let (Some(from), Some(to)) = (from_type, to_type) {
            if from.is_compatible(&to) {
                // Remove existing connection to this input
                self.connections
                    .retain(|c| c.to_node != to_node || c.to_socket != to_socket);

                self.connections.push(NodeConnection {
                    from_node,
                    from_socket,
                    to_node,
                    to_socket,
                });
                return true;
            }
        }
        false
    }

    /// Disconnect a socket.
    pub fn disconnect(&mut self, node_id: NodeId, socket_id: SocketId) {
        self.connections.retain(|c| {
            !((c.from_node == node_id && c.from_socket == socket_id)
                || (c.to_node == node_id && c.to_socket == socket_id))
        });
    }

    /// Select a node.
    pub fn select(&mut self, id: NodeId, add: bool) {
        if add {
            if !self.selected_nodes.contains(&id) {
                self.selected_nodes.push(id);
            }
        } else {
            self.selected_nodes.clear();
            self.selected_nodes.push(id);
        }
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selected_nodes.clear();
    }

    /// Check if node is selected.
    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected_nodes.contains(&id)
    }

    /// Get selected nodes.
    pub fn selected(&self) -> &[NodeId] {
        &self.selected_nodes
    }

    /// Move selected nodes.
    pub fn move_selected(&mut self, delta: [f32; 2]) {
        for &id in &self.selected_nodes {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.position[0] += delta[0];
                node.position[1] += delta[1];
            }
        }
    }

    /// Delete selected nodes.
    pub fn delete_selected(&mut self) {
        let to_delete: Vec<_> = self.selected_nodes.drain(..).collect();
        for id in to_delete {
            self.remove_node(id);
        }
    }

    /// Duplicate selected nodes.
    pub fn duplicate_selected(&mut self) {
        let mut new_ids = Vec::new();
        let mut id_map = HashMap::new();

        for &old_id in &self.selected_nodes.clone() {
            if let Some(old_node) = self.nodes.get(&old_id).cloned() {
                let new_id = self.next_node_id;
                self.next_node_id += 1;

                let mut new_node = old_node;
                new_node.id = new_id;
                new_node.position[0] += 50.0;
                new_node.position[1] += 50.0;

                // Remap socket IDs
                for socket in &mut new_node.inputs {
                    let old_socket_id = socket.id;
                    socket.id = self.next_socket_id;
                    self.next_socket_id += 1;
                    id_map.insert((old_id, old_socket_id), (new_id, socket.id));
                }
                for socket in &mut new_node.outputs {
                    let old_socket_id = socket.id;
                    socket.id = self.next_socket_id;
                    self.next_socket_id += 1;
                    id_map.insert((old_id, old_socket_id), (new_id, socket.id));
                }

                self.nodes.insert(new_id, new_node);
                new_ids.push(new_id);
            }
        }

        // Recreate connections between duplicated nodes
        let new_connections: Vec<_> = self
            .connections
            .iter()
            .filter_map(|c| {
                let from = id_map.get(&(c.from_node, c.from_socket))?;
                let to = id_map.get(&(c.to_node, c.to_socket))?;
                Some(NodeConnection {
                    from_node: from.0,
                    from_socket: from.1,
                    to_node: to.0,
                    to_socket: to.1,
                })
            })
            .collect();
        self.connections.extend(new_connections);

        // Select new nodes
        self.selected_nodes = new_ids;
    }

    /// Frame all nodes in view.
    pub fn frame_all(&mut self) {
        if self.nodes.is_empty() {
            self.offset = [0.0, 0.0];
            return;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for node in self.nodes.values() {
            min_x = min_x.min(node.position[0]);
            min_y = min_y.min(node.position[1]);
            max_x = max_x.max(node.position[0] + node.size[0]);
            max_y = max_y.max(node.position[1] + node.size[1]);
        }

        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        self.offset = [-center_x, -center_y];
    }

    /// Node count.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Connection count.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Add a node from a template, placing it at `graph_pos`.
    pub fn add_node_from_template(
        &mut self,
        template: &NodeTemplate,
        graph_pos: [f32; 2],
    ) -> NodeId {
        let id = self.add_node(&template.name, template.category);
        let height =
            30.0 + 25.0 * (template.inputs.len().max(template.outputs.len()) as f32) + 20.0;
        if let Some(node) = self.get_node_mut(id) {
            node.position = graph_pos;
            node.size = [200.0, height];
        }
        let n_sockets = template.inputs.len() + template.outputs.len();
        let mut sids = Vec::with_capacity(n_sockets);
        for _ in 0..n_sockets {
            sids.push(self.next_socket_id());
        }
        for (i, (name, stype)) in template.inputs.iter().enumerate() {
            if let Some(node) = self.get_node_mut(id) {
                node.add_input(sids[i], name, *stype);
            }
        }
        for (i, (name, stype)) in template.outputs.iter().enumerate() {
            if let Some(node) = self.get_node_mut(id) {
                node.add_output(sids[template.inputs.len() + i], name, *stype);
            }
        }
        id
    }
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// Node template for creating nodes from menus.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeTemplate {
    pub name: String,
    pub category: NodeCategory,
    pub inputs: Vec<(String, SocketType)>,
    pub outputs: Vec<(String, SocketType)>,
}

/// Get available node templates for material editor.
#[allow(dead_code)]
pub fn material_node_templates() -> Vec<NodeTemplate> {
    vec![
        // Input nodes
        NodeTemplate {
            name: "Texture Coordinate".to_string(),
            category: NodeCategory::Input,
            inputs: vec![],
            outputs: vec![
                ("Generated".to_string(), SocketType::Vector3),
                ("UV".to_string(), SocketType::Vector3),
                ("Object".to_string(), SocketType::Vector3),
                ("Camera".to_string(), SocketType::Vector3),
            ],
        },
        NodeTemplate {
            name: "Value".to_string(),
            category: NodeCategory::Input,
            inputs: vec![],
            outputs: vec![("Value".to_string(), SocketType::Float)],
        },
        NodeTemplate {
            name: "RGB".to_string(),
            category: NodeCategory::Input,
            inputs: vec![],
            outputs: vec![("Color".to_string(), SocketType::Color)],
        },
        // Shader nodes
        NodeTemplate {
            name: "Principled BSDF".to_string(),
            category: NodeCategory::Shader,
            inputs: vec![
                ("Base Color".to_string(), SocketType::Color),
                ("Metallic".to_string(), SocketType::Float),
                ("Roughness".to_string(), SocketType::Float),
                ("IOR".to_string(), SocketType::Float),
                ("Alpha".to_string(), SocketType::Float),
                ("Normal".to_string(), SocketType::Vector3),
            ],
            outputs: vec![("BSDF".to_string(), SocketType::Shader)],
        },
        NodeTemplate {
            name: "Diffuse BSDF".to_string(),
            category: NodeCategory::Shader,
            inputs: vec![
                ("Color".to_string(), SocketType::Color),
                ("Roughness".to_string(), SocketType::Float),
                ("Normal".to_string(), SocketType::Vector3),
            ],
            outputs: vec![("BSDF".to_string(), SocketType::Shader)],
        },
        NodeTemplate {
            name: "Glossy BSDF".to_string(),
            category: NodeCategory::Shader,
            inputs: vec![
                ("Color".to_string(), SocketType::Color),
                ("Roughness".to_string(), SocketType::Float),
                ("Normal".to_string(), SocketType::Vector3),
            ],
            outputs: vec![("BSDF".to_string(), SocketType::Shader)],
        },
        NodeTemplate {
            name: "Emission".to_string(),
            category: NodeCategory::Shader,
            inputs: vec![
                ("Color".to_string(), SocketType::Color),
                ("Strength".to_string(), SocketType::Float),
            ],
            outputs: vec![("Emission".to_string(), SocketType::Shader)],
        },
        NodeTemplate {
            name: "Mix Shader".to_string(),
            category: NodeCategory::Shader,
            inputs: vec![
                ("Fac".to_string(), SocketType::Float),
                ("Shader".to_string(), SocketType::Shader),
                ("Shader".to_string(), SocketType::Shader),
            ],
            outputs: vec![("Shader".to_string(), SocketType::Shader)],
        },
        // Texture nodes
        NodeTemplate {
            name: "Image Texture".to_string(),
            category: NodeCategory::Texture,
            inputs: vec![("Vector".to_string(), SocketType::Vector3)],
            outputs: vec![
                ("Color".to_string(), SocketType::Color),
                ("Alpha".to_string(), SocketType::Float),
            ],
        },
        NodeTemplate {
            name: "Noise Texture".to_string(),
            category: NodeCategory::Texture,
            inputs: vec![
                ("Vector".to_string(), SocketType::Vector3),
                ("Scale".to_string(), SocketType::Float),
                ("Detail".to_string(), SocketType::Float),
            ],
            outputs: vec![
                ("Fac".to_string(), SocketType::Float),
                ("Color".to_string(), SocketType::Color),
            ],
        },
        NodeTemplate {
            name: "Voronoi Texture".to_string(),
            category: NodeCategory::Texture,
            inputs: vec![
                ("Vector".to_string(), SocketType::Vector3),
                ("Scale".to_string(), SocketType::Float),
            ],
            outputs: vec![
                ("Distance".to_string(), SocketType::Float),
                ("Color".to_string(), SocketType::Color),
            ],
        },
        // Color nodes
        NodeTemplate {
            name: "Mix RGB".to_string(),
            category: NodeCategory::Color,
            inputs: vec![
                ("Fac".to_string(), SocketType::Float),
                ("Color1".to_string(), SocketType::Color),
                ("Color2".to_string(), SocketType::Color),
            ],
            outputs: vec![("Color".to_string(), SocketType::Color)],
        },
        NodeTemplate {
            name: "RGB Curves".to_string(),
            category: NodeCategory::Color,
            inputs: vec![
                ("Fac".to_string(), SocketType::Float),
                ("Color".to_string(), SocketType::Color),
            ],
            outputs: vec![("Color".to_string(), SocketType::Color)],
        },
        NodeTemplate {
            name: "Hue Saturation Value".to_string(),
            category: NodeCategory::Color,
            inputs: vec![
                ("Hue".to_string(), SocketType::Float),
                ("Saturation".to_string(), SocketType::Float),
                ("Value".to_string(), SocketType::Float),
                ("Fac".to_string(), SocketType::Float),
                ("Color".to_string(), SocketType::Color),
            ],
            outputs: vec![("Color".to_string(), SocketType::Color)],
        },
        // Vector nodes
        NodeTemplate {
            name: "Normal Map".to_string(),
            category: NodeCategory::Vector,
            inputs: vec![
                ("Strength".to_string(), SocketType::Float),
                ("Color".to_string(), SocketType::Color),
            ],
            outputs: vec![("Normal".to_string(), SocketType::Vector3)],
        },
        NodeTemplate {
            name: "Bump".to_string(),
            category: NodeCategory::Vector,
            inputs: vec![
                ("Strength".to_string(), SocketType::Float),
                ("Distance".to_string(), SocketType::Float),
                ("Height".to_string(), SocketType::Float),
                ("Normal".to_string(), SocketType::Vector3),
            ],
            outputs: vec![("Normal".to_string(), SocketType::Vector3)],
        },
        // Math nodes
        NodeTemplate {
            name: "Math".to_string(),
            category: NodeCategory::Math,
            inputs: vec![
                ("Value".to_string(), SocketType::Float),
                ("Value".to_string(), SocketType::Float),
            ],
            outputs: vec![("Value".to_string(), SocketType::Float)],
        },
        NodeTemplate {
            name: "Vector Math".to_string(),
            category: NodeCategory::Math,
            inputs: vec![
                ("Vector".to_string(), SocketType::Vector3),
                ("Vector".to_string(), SocketType::Vector3),
            ],
            outputs: vec![
                ("Vector".to_string(), SocketType::Vector3),
                ("Value".to_string(), SocketType::Float),
            ],
        },
        // Converter nodes
        NodeTemplate {
            name: "Color Ramp".to_string(),
            category: NodeCategory::Converter,
            inputs: vec![("Fac".to_string(), SocketType::Float)],
            outputs: vec![
                ("Color".to_string(), SocketType::Color),
                ("Alpha".to_string(), SocketType::Float),
            ],
        },
        NodeTemplate {
            name: "Separate RGB".to_string(),
            category: NodeCategory::Converter,
            inputs: vec![("Image".to_string(), SocketType::Color)],
            outputs: vec![
                ("R".to_string(), SocketType::Float),
                ("G".to_string(), SocketType::Float),
                ("B".to_string(), SocketType::Float),
            ],
        },
        NodeTemplate {
            name: "Combine RGB".to_string(),
            category: NodeCategory::Converter,
            inputs: vec![
                ("R".to_string(), SocketType::Float),
                ("G".to_string(), SocketType::Float),
                ("B".to_string(), SocketType::Float),
            ],
            outputs: vec![("Image".to_string(), SocketType::Color)],
        },
    ]
}

/// Get available node templates for geometry nodes.
#[allow(dead_code)]
pub fn geometry_node_templates() -> Vec<NodeTemplate> {
    vec![
        NodeTemplate {
            name: "Group Input".to_string(),
            category: NodeCategory::Input,
            inputs: vec![],
            outputs: vec![("Geometry".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Group Output".to_string(),
            category: NodeCategory::Output,
            inputs: vec![("Geometry".to_string(), SocketType::Geometry)],
            outputs: vec![],
        },
        NodeTemplate {
            name: "Transform".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![
                ("Geometry".to_string(), SocketType::Geometry),
                ("Translation".to_string(), SocketType::Vector3),
                ("Rotation".to_string(), SocketType::Vector3),
                ("Scale".to_string(), SocketType::Vector3),
            ],
            outputs: vec![("Geometry".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Join Geometry".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![("Geometry".to_string(), SocketType::Geometry)],
            outputs: vec![("Geometry".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Mesh Boolean".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![
                ("Mesh 1".to_string(), SocketType::Geometry),
                ("Mesh 2".to_string(), SocketType::Geometry),
            ],
            outputs: vec![("Mesh".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Subdivide Mesh".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![
                ("Mesh".to_string(), SocketType::Geometry),
                ("Level".to_string(), SocketType::Int),
            ],
            outputs: vec![("Mesh".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Extrude Mesh".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![
                ("Mesh".to_string(), SocketType::Geometry),
                ("Offset".to_string(), SocketType::Vector3),
            ],
            outputs: vec![
                ("Mesh".to_string(), SocketType::Geometry),
                ("Top".to_string(), SocketType::Bool),
                ("Side".to_string(), SocketType::Bool),
            ],
        },
        NodeTemplate {
            name: "Set Position".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![
                ("Geometry".to_string(), SocketType::Geometry),
                ("Position".to_string(), SocketType::Vector3),
                ("Offset".to_string(), SocketType::Vector3),
            ],
            outputs: vec![("Geometry".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Mesh Primitive Cube".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![("Size".to_string(), SocketType::Vector3)],
            outputs: vec![("Mesh".to_string(), SocketType::Geometry)],
        },
        NodeTemplate {
            name: "Mesh Primitive Sphere".to_string(),
            category: NodeCategory::Geometry,
            inputs: vec![
                ("Radius".to_string(), SocketType::Float),
                ("Segments".to_string(), SocketType::Int),
                ("Rings".to_string(), SocketType::Int),
            ],
            outputs: vec![("Mesh".to_string(), SocketType::Geometry)],
        },
    ]
}
