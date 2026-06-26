//! `physical-instancing` — GPU instancing data preparation for repeated geometry.
//!
//! Prepares instance transforms and colors for efficient GPU instanced
//! rendering of patterns (linear, circular) and assemblies with repeated parts.

/// Per-instance data sent to the GPU.
///
/// Each instance has a 4x4 transform matrix (column-major) and an RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstanceData {
    /// 4x4 column-major transform matrix.
    pub transform: [f32; 16],
    /// RGBA color (each component 0.0–1.0).
    pub color: [f32; 4],
}

impl InstanceData {
    /// Create an instance with the identity transform and the given color.
    pub fn with_color(color: [f32; 4]) -> Self {
        Self {
            transform: IDENTITY_MATRIX,
            color,
        }
    }

    /// Create an instance from a translation and color.
    pub fn from_translation(tx: f32, ty: f32, tz: f32, color: [f32; 4]) -> Self {
        let mut transform = IDENTITY_MATRIX;
        transform[12] = tx;
        transform[13] = ty;
        transform[14] = tz;
        Self { transform, color }
    }
}

impl Default for InstanceData {
    fn default() -> Self {
        Self {
            transform: IDENTITY_MATRIX,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// 4x4 identity matrix in column-major order.
const IDENTITY_MATRIX: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];

/// A batch of instances sharing the same mesh.
#[derive(Debug, Clone)]
pub struct InstanceBatch {
    /// Identifier for the shared mesh geometry.
    pub mesh_id: u64,
    /// Per-instance data (transforms + colors).
    pub instances: Vec<InstanceData>,
}

impl InstanceBatch {
    /// Create a new batch for the given mesh.
    pub fn new(mesh_id: u64) -> Self {
        Self {
            mesh_id,
            instances: Vec::new(),
        }
    }

    /// Add an instance to this batch.
    pub fn push(&mut self, instance: InstanceData) {
        self.instances.push(instance);
    }

    /// Number of instances in this batch.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Whether this batch is empty.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}

/// Manages multiple instance batches and provides packed GPU-ready data.
#[derive(Debug, Clone, Default)]
pub struct InstanceBuffer {
    /// All batches in this buffer.
    pub batches: Vec<InstanceBatch>,
}

/// Size of a single `InstanceData` in bytes (16 floats + 4 floats = 80 bytes).
const INSTANCE_DATA_SIZE: usize = std::mem::size_of::<InstanceData>();

impl InstanceBuffer {
    /// Create a new empty instance buffer.
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
        }
    }

    /// Add a batch to the buffer.
    pub fn add_batch(&mut self, batch: InstanceBatch) {
        self.batches.push(batch);
    }

    /// Total number of instances across all batches.
    pub fn total_instances(&self) -> usize {
        self.batches.iter().map(|b| b.instances.len()).sum()
    }

    /// Pack all instance data for a specific batch into a contiguous `Vec<f32>`.
    ///
    /// Layout per instance: 16 floats (transform) + 4 floats (color) = 20 floats.
    pub fn pack_batch(&self, batch_index: usize) -> Option<Vec<f32>> {
        let batch = self.batches.get(batch_index)?;
        let mut data = Vec::with_capacity(batch.instances.len() * 20);
        for inst in &batch.instances {
            data.extend_from_slice(&inst.transform);
            data.extend_from_slice(&inst.color);
        }
        Some(data)
    }

    /// Pack all instance data for a specific batch into raw bytes.
    ///
    /// Suitable for direct GPU buffer upload.
    pub fn pack_batch_bytes(&self, batch_index: usize) -> Option<Vec<u8>> {
        let batch = self.batches.get(batch_index)?;
        let mut bytes = Vec::with_capacity(batch.instances.len() * INSTANCE_DATA_SIZE);
        for inst in &batch.instances {
            for f in &inst.transform {
                bytes.extend_from_slice(&f.to_le_bytes());
            }
            for f in &inst.color {
                bytes.extend_from_slice(&f.to_le_bytes());
            }
        }
        Some(bytes)
    }

    /// Clear all batches.
    pub fn clear(&mut self) {
        self.batches.clear();
    }
}

/// Generate instance data for a linear pattern.
///
/// Creates `count` instances spaced evenly along the direction given by
/// `spacing` (x, y, z increments per step). The first instance is at the origin.
pub fn prepare_linear_pattern(count: u32, spacing: [f32; 3]) -> Vec<InstanceData> {
    (0..count)
        .map(|i| {
            let t = i as f32;
            InstanceData::from_translation(
                spacing[0] * t,
                spacing[1] * t,
                spacing[2] * t,
                [1.0, 1.0, 1.0, 1.0],
            )
        })
        .collect()
}

/// Generate instance data for a circular pattern.
///
/// Creates `count` instances equally spaced around a circle of the given
/// `radius`, rotating about the specified `axis` (should be a unit vector).
/// The pattern lies in the plane perpendicular to `axis`.
pub fn prepare_circular_pattern(count: u32, radius: f32, axis: [f32; 3]) -> Vec<InstanceData> {
    if count == 0 {
        return Vec::new();
    }

    // Normalize axis.
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let ax = if len > 1e-8 {
        [axis[0] / len, axis[1] / len, axis[2] / len]
    } else {
        [0.0, 0.0, 1.0]
    };

    // Find two orthonormal vectors perpendicular to the axis.
    let u = if ax[0].abs() < 0.9 {
        // cross(ax, [1,0,0])
        let cross = [0.0, ax[2], -ax[1]];
        let cl = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        [cross[0] / cl, cross[1] / cl, cross[2] / cl]
    } else {
        // cross(ax, [0,1,0])
        let cross = [-ax[2], 0.0, ax[0]];
        let cl = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        [cross[0] / cl, cross[1] / cl, cross[2] / cl]
    };

    // v = cross(ax, u)
    let v = [
        ax[1] * u[2] - ax[2] * u[1],
        ax[2] * u[0] - ax[0] * u[2],
        ax[0] * u[1] - ax[1] * u[0],
    ];

    let angle_step = std::f32::consts::TAU / count as f32;

    (0..count)
        .map(|i| {
            let angle = angle_step * i as f32;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            // Position on circle: radius * (cos * u + sin * v)
            let tx = radius * (cos_a * u[0] + sin_a * v[0]);
            let ty = radius * (cos_a * u[1] + sin_a * v[1]);
            let tz = radius * (cos_a * u[2] + sin_a * v[2]);

            // Build rotation matrix: rotate about `axis` by `angle`.
            // Rodrigues' rotation formula as a 4x4 column-major matrix.
            let c = cos_a;
            let s = sin_a;
            let t = 1.0 - c;

            #[rustfmt::skip]
            let transform = [
                // Column 0
                t * ax[0] * ax[0] + c,
                t * ax[0] * ax[1] + s * ax[2],
                t * ax[0] * ax[2] - s * ax[1],
                0.0,
                // Column 1
                t * ax[0] * ax[1] - s * ax[2],
                t * ax[1] * ax[1] + c,
                t * ax[1] * ax[2] + s * ax[0],
                0.0,
                // Column 2
                t * ax[0] * ax[2] + s * ax[1],
                t * ax[1] * ax[2] - s * ax[0],
                t * ax[2] * ax[2] + c,
                0.0,
                // Column 3 (translation)
                tx,
                ty,
                tz,
                1.0,
            ];

            InstanceData {
                transform,
                color: [1.0, 1.0, 1.0, 1.0],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_pattern_count() {
        let instances = prepare_linear_pattern(5, [10.0, 0.0, 0.0]);
        assert_eq!(instances.len(), 5);
    }

    #[test]
    fn linear_pattern_spacing() {
        let instances = prepare_linear_pattern(4, [5.0, 0.0, 0.0]);
        // First instance at origin.
        assert!((instances[0].transform[12] - 0.0).abs() < 1e-6);
        // Second at 5.0.
        assert!((instances[1].transform[12] - 5.0).abs() < 1e-6);
        // Third at 10.0.
        assert!((instances[2].transform[12] - 10.0).abs() < 1e-6);
        // Fourth at 15.0.
        assert!((instances[3].transform[12] - 15.0).abs() < 1e-6);
    }

    #[test]
    fn linear_pattern_3d_spacing() {
        let instances = prepare_linear_pattern(3, [1.0, 2.0, 3.0]);
        // Instance 2: translation = (2, 4, 6).
        assert!((instances[2].transform[12] - 2.0).abs() < 1e-6);
        assert!((instances[2].transform[13] - 4.0).abs() < 1e-6);
        assert!((instances[2].transform[14] - 6.0).abs() < 1e-6);
    }

    #[test]
    fn circular_pattern_count() {
        let instances = prepare_circular_pattern(8, 10.0, [0.0, 0.0, 1.0]);
        assert_eq!(instances.len(), 8);
    }

    #[test]
    fn circular_pattern_positions_z_axis() {
        let instances = prepare_circular_pattern(4, 10.0, [0.0, 0.0, 1.0]);
        let eps = 1e-4;

        // 4 instances around Z-axis at radius 10, all in XY plane (z=0).
        for inst in &instances {
            assert!(inst.transform[14].abs() < eps, "z should be ~0");
        }

        // All at radius 10 from origin.
        for inst in &instances {
            let x = inst.transform[12];
            let y = inst.transform[13];
            let r = (x * x + y * y).sqrt();
            assert!((r - 10.0).abs() < eps, "radius should be 10, got {r}");
        }

        // 90-degree spacing: each pair of adjacent instances should be orthogonal.
        let dot_01 = instances[0].transform[12] * instances[1].transform[12]
            + instances[0].transform[13] * instances[1].transform[13];
        assert!(dot_01.abs() < eps, "adjacent instances should be 90 degrees apart");

        // Opposite instances should be diametrically opposed.
        let sum_02_x = instances[0].transform[12] + instances[2].transform[12];
        let sum_02_y = instances[0].transform[13] + instances[2].transform[13];
        assert!(sum_02_x.abs() < eps && sum_02_y.abs() < eps, "opposite instances should cancel");
    }

    #[test]
    fn circular_pattern_all_same_radius() {
        let instances = prepare_circular_pattern(12, 5.0, [0.0, 0.0, 1.0]);
        let eps = 1e-4;
        for inst in &instances {
            let x = inst.transform[12];
            let y = inst.transform[13];
            let z = inst.transform[14];
            let r = (x * x + y * y + z * z).sqrt();
            assert!(
                (r - 5.0).abs() < eps,
                "Instance at ({x}, {y}, {z}) has radius {r}, expected 5.0"
            );
        }
    }

    #[test]
    fn circular_pattern_zero_count() {
        let instances = prepare_circular_pattern(0, 10.0, [0.0, 0.0, 1.0]);
        assert!(instances.is_empty());
    }

    #[test]
    fn buffer_packing() {
        let mut buffer = InstanceBuffer::new();
        let mut batch = InstanceBatch::new(42);
        batch.push(InstanceData::from_translation(1.0, 2.0, 3.0, [1.0, 0.0, 0.0, 1.0]));
        batch.push(InstanceData::from_translation(4.0, 5.0, 6.0, [0.0, 1.0, 0.0, 1.0]));
        buffer.add_batch(batch);

        assert_eq!(buffer.total_instances(), 2);

        let packed = buffer.pack_batch(0).unwrap();
        // 2 instances * 20 floats = 40 floats.
        assert_eq!(packed.len(), 40);

        // Check first instance translation (indices 12..15 in the first 20 floats).
        assert!((packed[12] - 1.0).abs() < 1e-6);
        assert!((packed[13] - 2.0).abs() < 1e-6);
        assert!((packed[14] - 3.0).abs() < 1e-6);

        // Check first instance color (indices 16..20).
        assert!((packed[16] - 1.0).abs() < 1e-6);
        assert!((packed[17] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn buffer_packing_bytes() {
        let mut buffer = InstanceBuffer::new();
        let mut batch = InstanceBatch::new(1);
        batch.push(InstanceData::default());
        buffer.add_batch(batch);

        let bytes = buffer.pack_batch_bytes(0).unwrap();
        // 1 instance * 20 floats * 4 bytes = 80 bytes.
        assert_eq!(bytes.len(), 80);
    }

    #[test]
    fn buffer_invalid_batch_index() {
        let buffer = InstanceBuffer::new();
        assert!(buffer.pack_batch(0).is_none());
        assert!(buffer.pack_batch_bytes(0).is_none());
    }

    #[test]
    fn instance_data_default() {
        let d = InstanceData::default();
        assert_eq!(d.transform, IDENTITY_MATRIX);
        assert_eq!(d.color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn batch_operations() {
        let mut batch = InstanceBatch::new(99);
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);

        batch.push(InstanceData::default());
        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.mesh_id, 99);
    }
}
