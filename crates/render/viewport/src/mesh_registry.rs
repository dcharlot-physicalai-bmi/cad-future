//! GPU mesh registry — stores vertex/index buffers with handle-based access.

use wgpu::util::DeviceExt;

use crate::mesh;
use crate::vertex::Vertex;
use crate::wireframe::triangles_to_edges;

/// Unique mesh identifier.
pub type MeshId = u32;

/// Well-known built-in mesh IDs, registered automatically on construction.
pub mod builtin {
    use super::MeshId;
    pub const CUBE: MeshId = 0;
    pub const SPHERE: MeshId = 1;
    pub const CYLINDER: MeshId = 2;
    pub const TORUS: MeshId = 3;
    pub const ICOSPHERE: MeshId = 4;
}

struct GpuMesh {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: u32,
    edge_buf: wgpu::Buffer,
    edge_count: u32,
}

/// Registry that owns GPU buffers for all registered meshes.
pub struct MeshRegistry {
    meshes: Vec<GpuMesh>,
}

impl MeshRegistry {
    /// Create a new registry and pre-register the built-in CAD primitives.
    pub fn new(device: &wgpu::Device) -> Self {
        let mut reg = Self { meshes: Vec::new() };

        // Order must match builtin constants above.
        let primitives: Vec<(Vec<Vertex>, Vec<u32>)> = vec![
            mesh::cube(1.0),
            mesh::sphere(0.5, 32, 16),
            mesh::cylinder(0.5, 1.0, 32),
            mesh::torus(0.5, 0.2, 32, 16),
            mesh::icosphere(0.5, 2),
        ];

        for (verts, indices) in primitives {
            reg.register(device, &verts, &indices);
        }

        reg
    }

    /// Register a mesh from CPU vertex/index data and return its ID.
    pub fn register(
        &mut self,
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> MeshId {
        let id = self.meshes.len() as MeshId;

        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("mesh-{id}-vb")),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("mesh-{id}-ib")),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let edges = triangles_to_edges(indices);
        let edge_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("mesh-{id}-eb")),
            contents: bytemuck::cast_slice(&edges),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.meshes.push(GpuMesh {
            vertex_buf,
            index_buf,
            index_count: indices.len() as u32,
            edge_buf,
            edge_count: edges.len() as u32,
        });

        id
    }

    /// Get the GPU buffers for a mesh: (vertex_buffer, index_buffer, index_count).
    pub fn get(&self, id: MeshId) -> Option<(&wgpu::Buffer, &wgpu::Buffer, u32)> {
        self.meshes
            .get(id as usize)
            .map(|m| (&m.vertex_buf, &m.index_buf, m.index_count))
    }

    /// Get the edge (wireframe) buffers for a mesh: (vertex_buffer, edge_index_buffer, edge_count).
    pub fn get_edges(&self, id: MeshId) -> Option<(&wgpu::Buffer, &wgpu::Buffer, u32)> {
        self.meshes
            .get(id as usize)
            .map(|m| (&m.vertex_buf, &m.edge_buf, m.edge_count))
    }

    pub fn len(&self) -> usize {
        self.meshes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
    }
}
