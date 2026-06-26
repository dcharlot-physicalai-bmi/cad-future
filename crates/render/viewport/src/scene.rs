//! Scene graph — flat list of drawable objects for the CAD viewport.

use crate::material::MaterialId;
use crate::mesh_registry::MeshId;

/// A single object to be drawn.
#[derive(Clone, Debug)]
pub struct RenderObject {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
    pub transform: glam::Mat4,
    pub object_id: u32,
}

/// A node in the scene representing one drawable mesh instance.
#[derive(Clone, Debug)]
pub struct SceneNode {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
    pub transform: glam::Mat4,
    pub name: String,
    pub visible: bool,
}

impl SceneNode {
    pub fn new(name: &str, mesh_id: MeshId, material_id: MaterialId, transform: glam::Mat4) -> Self {
        Self {
            mesh_id,
            material_id,
            transform,
            name: name.to_string(),
            visible: true,
        }
    }
}

/// Flat scene that owns a list of SceneNodes.
/// Swap-remove for O(1) deletion.
pub struct Scene {
    nodes: Vec<SceneNode>,
}

impl Scene {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(&mut self, node: SceneNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    pub fn remove(&mut self, index: usize) {
        self.nodes.swap_remove(index);
    }

    /// Insert a node at a specific index (for undo).
    pub fn insert(&mut self, index: usize, node: SceneNode) {
        if index >= self.nodes.len() {
            self.nodes.push(node);
        } else {
            self.nodes.insert(index, node);
        }
    }

    pub fn set_transform(&mut self, index: usize, transform: glam::Mat4) {
        self.nodes[index].transform = transform;
    }

    pub fn to_render_objects(&self) -> Vec<RenderObject> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.visible)
            .map(|(i, node)| RenderObject {
                mesh_id: node.mesh_id,
                material_id: node.material_id,
                transform: node.transform,
                object_id: i as u32,
            })
            .collect()
    }

    /// Get a node by index.
    pub fn node(&self, index: usize) -> &SceneNode {
        &self.nodes[index]
    }

    /// Get a mutable node by index.
    pub fn node_mut(&mut self, index: usize) -> &mut SceneNode {
        &mut self.nodes[index]
    }

    /// Iterate over all nodes with their indices.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &SceneNode)> {
        self.nodes.iter().enumerate()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get a node's transform.
    pub fn transform(&self, index: usize) -> glam::Mat4 {
        self.nodes[index].transform
    }

    /// Ray-sphere pick: find the nearest node whose bounding sphere (radius 1.5,
    /// centered at the node's translation) is hit by the ray.
    /// Returns the node index.
    pub fn pick(&self, ray_origin: glam::Vec3, ray_dir: glam::Vec3) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, node) in self.nodes.iter().enumerate() {
            let center = node.transform.col(3).truncate();
            // Approximate bounding radius from scale
            let sx = node.transform.col(0).truncate().length();
            let sy = node.transform.col(1).truncate().length();
            let sz = node.transform.col(2).truncate().length();
            let radius = sx.max(sy).max(sz) * 1.2;

            if let Some(t) = ray_sphere(ray_origin, ray_dir, center, radius) {
                if t > 0.0 && best.map_or(true, |(_, bt)| t < bt) {
                    best = Some((i, t));
                }
            }
        }
        best.map(|(i, _)| i)
    }
}

/// Ray-sphere intersection, returns t parameter (distance along ray) or None.
fn ray_sphere(origin: glam::Vec3, dir: glam::Vec3, center: glam::Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let a = dir.dot(dir);
    let b = 2.0 * oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / (2.0 * a);
    if t > 0.0 { Some(t) } else { Some((-b + disc.sqrt()) / (2.0 * a)) }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort render objects by (mesh_id, material_id) for draw-call batching.
pub fn sorted_draw_order(objects: &[RenderObject]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..objects.len()).collect();
    indices.sort_by(|&a, &b| {
        let oa = &objects[a];
        let ob = &objects[b];
        (oa.mesh_id, oa.material_id).cmp(&(ob.mesh_id, ob.material_id))
    });
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    fn make_node(mesh: u32, mat: u32) -> SceneNode {
        SceneNode::new("test", mesh, mat, Mat4::IDENTITY)
    }

    #[test]
    fn new_scene_is_empty() {
        let scene = Scene::new();
        assert!(scene.is_empty());
        assert_eq!(scene.len(), 0);
    }

    #[test]
    fn add_and_remove() {
        let mut scene = Scene::new();
        scene.add(make_node(0, 0));
        scene.add(make_node(1, 1));
        assert_eq!(scene.len(), 2);
        scene.remove(0);
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn to_render_objects_preserves_data() {
        let mut scene = Scene::new();
        let t = Mat4::from_scale(glam::Vec3::new(2.0, 2.0, 2.0));
        scene.add(SceneNode::new("test", 5, 3, t));
        let objects = scene.to_render_objects();
        assert_eq!(objects[0].mesh_id, 5);
        assert_eq!(objects[0].material_id, 3);
        assert_eq!(objects[0].transform, t);
    }

    #[test]
    fn sorted_draw_order_groups() {
        let objects = vec![
            RenderObject { mesh_id: 1, material_id: 2, transform: Mat4::IDENTITY, object_id: 0 },
            RenderObject { mesh_id: 0, material_id: 1, transform: Mat4::IDENTITY, object_id: 1 },
            RenderObject { mesh_id: 1, material_id: 0, transform: Mat4::IDENTITY, object_id: 2 },
            RenderObject { mesh_id: 0, material_id: 0, transform: Mat4::IDENTITY, object_id: 3 },
        ];
        let order = sorted_draw_order(&objects);
        assert_eq!(order, vec![3, 1, 2, 0]);
    }

    #[test]
    fn clear_empties_scene() {
        let mut scene = Scene::new();
        scene.add(make_node(0, 0));
        scene.clear();
        assert!(scene.is_empty());
    }
}
