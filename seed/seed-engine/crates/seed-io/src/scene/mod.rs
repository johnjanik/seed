//! UnifiedScene: The central scene representation for format interchange.

pub mod geometry;
pub mod material;
pub mod metadata;

pub use geometry::*;
pub use material::*;
pub use metadata::*;

use glam::Mat4;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The central scene representation for format interchange.
///
/// UnifiedScene serves as the hub for converting between formats:
/// - Seed documents
/// - glTF 2.0 files
/// - STEP CAD files
/// - USD scenes
///
/// All format readers convert to UnifiedScene, and all writers convert from it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnifiedScene {
    /// Scene nodes (hierarchy).
    pub nodes: Vec<SceneNode>,
    /// Root node indices.
    pub roots: Vec<usize>,
    /// Geometry data.
    pub geometries: Vec<Geometry>,
    /// Materials.
    pub materials: Vec<Material>,
    /// Textures.
    pub textures: Vec<Texture>,
    /// Scene metadata.
    pub metadata: SceneMetadata,
    /// Format-specific extensions (preserved for round-trip).
    pub extensions: ExtensionData,
}

impl UnifiedScene {
    /// Create a new empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the total number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the total number of geometries.
    pub fn geometry_count(&self) -> usize {
        self.geometries.len()
    }

    /// Add a root node and return its index.
    pub fn add_root(&mut self, node: SceneNode) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        self.roots.push(index);
        index
    }

    /// Add a child node to a parent and return its index.
    pub fn add_child(&mut self, parent: usize, node: SceneNode) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        if parent < self.nodes.len() {
            self.nodes[parent].children.push(index);
        }
        index
    }

    /// Add a geometry and return its index.
    pub fn add_geometry(&mut self, geometry: Geometry) -> usize {
        let index = self.geometries.len();
        self.geometries.push(geometry);
        index
    }

    /// Add a material and return its index.
    pub fn add_material(&mut self, material: Material) -> usize {
        let index = self.materials.len();
        self.materials.push(material);
        index
    }

    /// Add a texture and return its index.
    pub fn add_texture(&mut self, texture: Texture) -> usize {
        let index = self.textures.len();
        self.textures.push(texture);
        index
    }

    /// Compute the scene bounding box.
    pub fn compute_bounds(&self) -> BoundingBox {
        let mut bounds = BoundingBox::default();
        for node in &self.nodes {
            if let Some(geom_idx) = node.geometry {
                if let Some(geom) = self.geometries.get(geom_idx) {
                    let node_bounds = geom.bounds();
                    // TODO: transform by node.transform
                    bounds.expand(&node_bounds);
                }
            }
        }
        bounds
    }

    /// Iterate over all nodes with their transforms.
    pub fn traverse(&self) -> impl Iterator<Item = (usize, &SceneNode, Mat4)> {
        SceneTraverser::new(self)
    }
}

/// A node in the scene graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneNode {
    /// Node name.
    pub name: String,
    /// Local transform.
    pub transform: Mat4,
    /// Child node indices.
    pub children: Vec<usize>,
    /// Geometry index (if this node has geometry).
    pub geometry: Option<usize>,
    /// Material index (if this node has a material override).
    pub material: Option<usize>,
    /// Whether this node is visible.
    pub visible: bool,
    /// Node metadata.
    pub metadata: NodeMetadata,
}

impl SceneNode {
    /// Create a new named node.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transform: Mat4::IDENTITY,
            visible: true,
            ..Default::default()
        }
    }

    /// Create a node with geometry.
    pub fn with_geometry(name: impl Into<String>, geometry: usize) -> Self {
        Self {
            name: name.into(),
            transform: Mat4::IDENTITY,
            geometry: Some(geometry),
            visible: true,
            ..Default::default()
        }
    }

    /// Set the transform.
    pub fn transformed(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    /// Set the material.
    pub fn with_material(mut self, material: usize) -> Self {
        self.material = Some(material);
        self
    }
}

/// Format-specific extension data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionData {
    /// glTF extensions.
    pub gltf: IndexMap<String, serde_json::Value>,
    /// USD layer data.
    pub usd: IndexMap<String, serde_json::Value>,
    /// STEP entity references.
    pub step: IndexMap<String, serde_json::Value>,
    /// Seed-specific data.
    pub seed: IndexMap<String, serde_json::Value>,
}

/// Iterator for traversing the scene graph.
struct SceneTraverser<'a> {
    scene: &'a UnifiedScene,
    stack: Vec<(usize, Mat4)>,
}

impl<'a> SceneTraverser<'a> {
    fn new(scene: &'a UnifiedScene) -> Self {
        let stack: Vec<(usize, Mat4)> = scene
            .roots
            .iter()
            .rev()
            .map(|&idx| (idx, Mat4::IDENTITY))
            .collect();
        Self { scene, stack }
    }
}

impl<'a> Iterator for SceneTraverser<'a> {
    type Item = (usize, &'a SceneNode, Mat4);

    fn next(&mut self) -> Option<Self::Item> {
        let (idx, parent_transform) = self.stack.pop()?;
        let node = &self.scene.nodes[idx];
        let world_transform = parent_transform * node.transform;

        // Push children in reverse order so they're processed left-to-right
        for &child_idx in node.children.iter().rev() {
            self.stack.push((child_idx, world_transform));
        }

        Some((idx, node, world_transform))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_empty_scene() {
        let scene = UnifiedScene::new();
        assert_eq!(scene.node_count(), 0);
        assert_eq!(scene.geometry_count(), 0);
    }

    #[test]
    fn test_add_root() {
        let mut scene = UnifiedScene::new();
        let idx = scene.add_root(SceneNode::new("root"));
        assert_eq!(idx, 0);
        assert_eq!(scene.roots, vec![0]);
    }

    #[test]
    fn test_scene_traversal() {
        let mut scene = UnifiedScene::new();
        let root = scene.add_root(SceneNode::new("root"));
        let child1 = scene.add_child(root, SceneNode::new("child1"));
        let _child2 = scene.add_child(root, SceneNode::new("child2"));
        let _grandchild = scene.add_child(child1, SceneNode::new("grandchild"));

        let names: Vec<&str> = scene.traverse().map(|(_, n, _)| n.name.as_str()).collect();
        assert_eq!(names, vec!["root", "child1", "grandchild", "child2"]);
    }

    #[test]
    fn test_bounding_box() {
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(-1.0, -1.0, -1.0),
        ];
        let bounds = BoundingBox::from_points(&points);
        assert_eq!(bounds.min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(bounds.max, Vec3::new(1.0, 2.0, 3.0));
    }
}
