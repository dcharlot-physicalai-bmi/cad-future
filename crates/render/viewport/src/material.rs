//! Material system for the CAD renderer.
//!
//! Flat uniform materials: base color, roughness, metallic.
//! No textures in Phase 1 — engineering materials are defined by properties, not images.

/// Unique material identifier.
pub type MaterialId = u32;

/// Surface material for CAD objects.
#[derive(Clone, Debug)]
pub struct Material {
    pub base_color: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [0.8, 0.8, 0.8, 1.0],
            roughness: 0.5,
            metallic: 0.0,
        }
    }
}

/// Named material presets for common engineering materials.
impl Material {
    pub fn aluminum() -> Self {
        Self {
            base_color: [0.85, 0.87, 0.90, 1.0],
            roughness: 0.35,
            metallic: 1.0,
        }
    }

    pub fn steel() -> Self {
        Self {
            base_color: [0.56, 0.57, 0.58, 1.0],
            roughness: 0.4,
            metallic: 1.0,
        }
    }

    pub fn titanium() -> Self {
        Self {
            base_color: [0.62, 0.58, 0.55, 1.0],
            roughness: 0.3,
            metallic: 1.0,
        }
    }

    pub fn brass() -> Self {
        Self {
            base_color: [0.78, 0.67, 0.22, 1.0],
            roughness: 0.25,
            metallic: 1.0,
        }
    }

    pub fn plastic_white() -> Self {
        Self {
            base_color: [0.95, 0.95, 0.93, 1.0],
            roughness: 0.7,
            metallic: 0.0,
        }
    }

    pub fn plastic_black() -> Self {
        Self {
            base_color: [0.05, 0.05, 0.05, 1.0],
            roughness: 0.6,
            metallic: 0.0,
        }
    }

    /// Bright red for constraint violations / error highlighting.
    pub fn error() -> Self {
        Self {
            base_color: [0.9, 0.15, 0.15, 1.0],
            roughness: 0.5,
            metallic: 0.0,
        }
    }

    /// Green for pass / valid state.
    pub fn valid() -> Self {
        Self {
            base_color: [0.15, 0.8, 0.25, 1.0],
            roughness: 0.5,
            metallic: 0.0,
        }
    }
}

/// Stores materials and provides ID-based access.
pub struct MaterialStore {
    materials: Vec<Material>,
}

impl MaterialStore {
    pub fn new() -> Self {
        let mut store = Self { materials: Vec::new() };
        store.materials.push(Material::default());
        store
    }

    pub fn add(&mut self, mat: Material) -> MaterialId {
        let id = self.materials.len() as MaterialId;
        self.materials.push(mat);
        id
    }

    pub fn get(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(id as usize)
    }

    pub fn default_id(&self) -> MaterialId {
        0
    }

    pub fn len(&self) -> usize {
        self.materials.len()
    }

    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }
}

impl Default for MaterialStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_material_values() {
        let m = Material::default();
        assert_eq!(m.base_color, [0.8, 0.8, 0.8, 1.0]);
        assert!((m.roughness - 0.5).abs() < f32::EPSILON);
        assert!((m.metallic).abs() < f32::EPSILON);
    }

    #[test]
    fn store_starts_with_default() {
        let store = MaterialStore::new();
        assert_eq!(store.len(), 1);
        assert_eq!(store.default_id(), 0);
        assert!(store.get(0).is_some());
    }

    #[test]
    fn store_add_and_get() {
        let mut store = MaterialStore::new();
        let id = store.add(Material::aluminum());
        assert_eq!(id, 1);
        let m = store.get(id).unwrap();
        assert!(m.metallic > 0.9);
    }

    #[test]
    fn store_out_of_bounds() {
        let store = MaterialStore::new();
        assert!(store.get(999).is_none());
    }

    #[test]
    fn preset_materials_valid() {
        let presets = [
            Material::aluminum(),
            Material::steel(),
            Material::titanium(),
            Material::brass(),
            Material::plastic_white(),
            Material::plastic_black(),
            Material::error(),
            Material::valid(),
        ];
        for m in &presets {
            assert!(m.base_color[3] > 0.0);
            assert!(m.roughness >= 0.0 && m.roughness <= 1.0);
            assert!(m.metallic >= 0.0 && m.metallic <= 1.0);
        }
    }
}
