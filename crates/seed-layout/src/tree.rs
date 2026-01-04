//! Layout tree data structures.
//!
//! The layout tree mirrors the document structure but contains computed
//! positions and sizes for each element.

use std::collections::HashMap;
use glam::Vec2;
use seed_core::types::ElementId;

/// Unique identifier for a layout node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutNodeId(pub u64);

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bounds {
    /// Position relative to parent (or absolute if root)
    pub x: f64,
    pub y: f64,
    /// Size of the element
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    /// Create bounds with position and size.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Create bounds from position and size vectors.
    pub fn from_vecs(position: Vec2, size: Vec2) -> Self {
        Self {
            x: position.x as f64,
            y: position.y as f64,
            width: size.x as f64,
            height: size.y as f64,
        }
    }

    /// Get position as Vec2.
    pub fn position(&self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }

    /// Get size as Vec2.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }

    /// Get the right edge (x + width).
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// Get the bottom edge (y + height).
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Get the center X coordinate.
    pub fn center_x(&self) -> f64 {
        self.x + self.width / 2.0
    }

    /// Get the center Y coordinate.
    pub fn center_y(&self) -> f64 {
        self.y + self.height / 2.0
    }

    /// Check if a point is inside the bounds.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Compute intersection with another bounds.
    pub fn intersect(&self, other: &Bounds) -> Option<Bounds> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());

        if x1 < x2 && y1 < y2 {
            Some(Bounds::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }

    /// Compute union (bounding box) with another bounds.
    pub fn union(&self, other: &Bounds) -> Bounds {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = self.right().max(other.right());
        let y2 = self.bottom().max(other.bottom());
        Bounds::new(x1, y1, x2 - x1, y2 - y1)
    }

    /// Expand bounds by a uniform amount.
    pub fn expand(&self, amount: f64) -> Bounds {
        Bounds::new(
            self.x - amount,
            self.y - amount,
            self.width + 2.0 * amount,
            self.height + 2.0 * amount,
        )
    }

    /// Inset bounds by a uniform amount.
    pub fn inset(&self, amount: f64) -> Bounds {
        self.expand(-amount)
    }
}

/// A node in the layout tree.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    /// Unique ID for this node
    pub id: LayoutNodeId,
    /// Optional element ID from the source document
    pub element_id: Option<ElementId>,
    /// Optional name for debugging
    pub name: Option<String>,
    /// Computed bounds (position relative to parent)
    pub bounds: Bounds,
    /// Absolute bounds (position in document coordinates)
    pub absolute_bounds: Bounds,
    /// Parent node ID (None for root)
    pub parent: Option<LayoutNodeId>,
    /// Child node IDs
    pub children: Vec<LayoutNodeId>,
    /// Clip children to this node's bounds
    pub clips_children: bool,
    /// Opacity (0.0 to 1.0)
    pub opacity: f64,
    /// Whether this node is visible
    pub visible: bool,
}

impl LayoutNode {
    /// Create a new layout node.
    pub fn new(id: LayoutNodeId) -> Self {
        Self {
            id,
            element_id: None,
            name: None,
            bounds: Bounds::default(),
            absolute_bounds: Bounds::default(),
            parent: None,
            children: Vec::new(),
            clips_children: false,
            opacity: 1.0,
            visible: true,
        }
    }

    /// Set the element ID.
    pub fn with_element_id(mut self, element_id: ElementId) -> Self {
        self.element_id = Some(element_id);
        self
    }

    /// Set the name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the bounds.
    pub fn with_bounds(mut self, bounds: Bounds) -> Self {
        self.bounds = bounds;
        self
    }
}

/// The complete layout tree for a document.
#[derive(Debug, Clone)]
pub struct LayoutTree {
    /// All nodes in the tree, indexed by ID
    nodes: HashMap<LayoutNodeId, LayoutNode>,
    /// Root node IDs (top-level elements)
    roots: Vec<LayoutNodeId>,
    /// Counter for generating unique IDs
    next_id: u64,
    /// Mapping from element IDs to layout node IDs
    element_to_node: HashMap<ElementId, LayoutNodeId>,
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutTree {
    /// Create an empty layout tree.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            roots: Vec::new(),
            next_id: 0,
            element_to_node: HashMap::new(),
        }
    }

    /// Generate a new unique node ID.
    pub fn next_id(&mut self) -> LayoutNodeId {
        let id = LayoutNodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Add a root node.
    pub fn add_root(&mut self, node: LayoutNode) -> LayoutNodeId {
        let id = node.id;
        if let Some(element_id) = node.element_id {
            self.element_to_node.insert(element_id, id);
        }
        self.nodes.insert(id, node);
        self.roots.push(id);
        id
    }

    /// Add a child node to a parent.
    pub fn add_child(&mut self, parent_id: LayoutNodeId, mut node: LayoutNode) -> LayoutNodeId {
        let id = node.id;
        node.parent = Some(parent_id);

        if let Some(element_id) = node.element_id {
            self.element_to_node.insert(element_id, id);
        }

        self.nodes.insert(id, node);

        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(id);
        }

        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: LayoutNodeId) -> Option<&LayoutNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, id: LayoutNodeId) -> Option<&mut LayoutNode> {
        self.nodes.get_mut(&id)
    }

    /// Get a node by element ID.
    pub fn get_by_element(&self, element_id: ElementId) -> Option<&LayoutNode> {
        self.element_to_node
            .get(&element_id)
            .and_then(|id| self.nodes.get(id))
    }

    /// Get the root nodes.
    pub fn roots(&self) -> &[LayoutNodeId] {
        &self.roots
    }

    /// Iterate over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &LayoutNode> {
        self.nodes.values()
    }

    /// Get children of a node.
    pub fn children(&self, id: LayoutNodeId) -> impl Iterator<Item = &LayoutNode> {
        self.nodes
            .get(&id)
            .into_iter()
            .flat_map(|n| n.children.iter())
            .filter_map(|child_id| self.nodes.get(child_id))
    }

    /// Compute absolute bounds for all nodes.
    pub fn compute_absolute_bounds(&mut self) {
        // Process root nodes first
        for root_id in self.roots.clone() {
            self.compute_absolute_bounds_recursive(root_id, 0.0, 0.0);
        }
    }

    fn compute_absolute_bounds_recursive(&mut self, id: LayoutNodeId, parent_x: f64, parent_y: f64) {
        let (abs_x, abs_y, children) = {
            let node = match self.nodes.get_mut(&id) {
                Some(n) => n,
                None => return,
            };
            let abs_x = parent_x + node.bounds.x;
            let abs_y = parent_y + node.bounds.y;
            node.absolute_bounds = Bounds::new(
                abs_x,
                abs_y,
                node.bounds.width,
                node.bounds.height,
            );
            (abs_x, abs_y, node.children.clone())
        };

        for child_id in children {
            self.compute_absolute_bounds_recursive(child_id, abs_x, abs_y);
        }
    }

    /// Find the node at a given point (in absolute coordinates).
    pub fn hit_test(&self, x: f64, y: f64) -> Option<LayoutNodeId> {
        // Test root nodes in reverse order (last one is on top)
        for &root_id in self.roots.iter().rev() {
            if let Some(hit) = self.hit_test_recursive(root_id, x, y) {
                return Some(hit);
            }
        }
        None
    }

    fn hit_test_recursive(&self, id: LayoutNodeId, x: f64, y: f64) -> Option<LayoutNodeId> {
        let node = self.nodes.get(&id)?;

        if !node.visible {
            return None;
        }

        if !node.absolute_bounds.contains(x, y) {
            return None;
        }

        // Test children in reverse order (last one is on top)
        for &child_id in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_recursive(child_id, x, y) {
                return Some(hit);
            }
        }

        Some(id)
    }

    /// Get the total bounds of all content.
    pub fn content_bounds(&self) -> Bounds {
        let mut result = Bounds::default();
        let mut first = true;

        for &root_id in &self.roots {
            if let Some(node) = self.nodes.get(&root_id) {
                if first {
                    result = node.absolute_bounds;
                    first = false;
                } else {
                    result = result.union(&node.absolute_bounds);
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_contains() {
        let bounds = Bounds::new(10.0, 20.0, 100.0, 50.0);
        assert!(bounds.contains(50.0, 40.0));
        assert!(!bounds.contains(5.0, 40.0));
        assert!(!bounds.contains(50.0, 100.0));
    }

    #[test]
    fn test_bounds_intersection() {
        let a = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let b = Bounds::new(50.0, 50.0, 100.0, 100.0);
        let intersection = a.intersect(&b).unwrap();
        assert!((intersection.x - 50.0).abs() < 0.001);
        assert!((intersection.y - 50.0).abs() < 0.001);
        assert!((intersection.width - 50.0).abs() < 0.001);
        assert!((intersection.height - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_tree() {
        let mut tree = LayoutTree::new();

        let root_id = tree.next_id();
        let root = LayoutNode::new(root_id)
            .with_name("root")
            .with_bounds(Bounds::new(0.0, 0.0, 800.0, 600.0));
        tree.add_root(root);

        let child_id = tree.next_id();
        let child = LayoutNode::new(child_id)
            .with_name("child")
            .with_bounds(Bounds::new(10.0, 10.0, 100.0, 50.0));
        tree.add_child(root_id, child);

        tree.compute_absolute_bounds();

        let child_node = tree.get(child_id).unwrap();
        assert!((child_node.absolute_bounds.x - 10.0).abs() < 0.001);
        assert!((child_node.absolute_bounds.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_hit_test() {
        let mut tree = LayoutTree::new();

        let root_id = tree.next_id();
        let root = LayoutNode::new(root_id)
            .with_bounds(Bounds::new(0.0, 0.0, 800.0, 600.0));
        tree.add_root(root);

        let child_id = tree.next_id();
        let child = LayoutNode::new(child_id)
            .with_bounds(Bounds::new(100.0, 100.0, 200.0, 100.0));
        tree.add_child(root_id, child);

        tree.compute_absolute_bounds();

        // Hit child
        assert_eq!(tree.hit_test(150.0, 150.0), Some(child_id));
        // Hit root (outside child)
        assert_eq!(tree.hit_test(50.0, 50.0), Some(root_id));
        // Miss everything
        assert_eq!(tree.hit_test(1000.0, 1000.0), None);
    }
}
