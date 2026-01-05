//! 3D scene building from documents.

use glam::{Vec3, Mat4};
use seed_core::{Document, ast::{Element, PartElement, Property, PropertyValue}, types::Color};

use crate::geometry::{Shape, Mesh, BoundingBox};
use crate::material::{Material, Light};
use crate::tessellation::{tessellate, TessellationOptions, tessellate_with_options};

/// A 3D scene containing objects, lights, and camera.
#[derive(Debug, Clone)]
pub struct Scene3D {
    /// Objects in the scene.
    pub objects: Vec<SceneObject>,
    /// Lights in the scene.
    pub lights: Vec<Light>,
    /// Camera settings.
    pub camera: Camera,
    /// Background color.
    pub background: Color,
}

impl Default for Scene3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene3D {
    /// Create a new empty scene.
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            lights: vec![
                Light::default_sun(),
                Light::default_ambient(),
            ],
            camera: Camera::default(),
            background: Color::rgb(0.1, 0.1, 0.15),
        }
    }

    /// Add an object to the scene.
    pub fn add_object(&mut self, object: SceneObject) {
        self.objects.push(object);
    }

    /// Add a light to the scene.
    pub fn add_light(&mut self, light: Light) {
        self.lights.push(light);
    }

    /// Get the combined bounding box of all objects.
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        self.objects.iter()
            .filter_map(|obj| obj.mesh.bounding_box())
            .reduce(|a, b| a.union(&b))
    }

    /// Auto-fit camera to see all objects.
    pub fn fit_camera(&mut self) {
        if let Some(bounds) = self.bounding_box() {
            let center = bounds.center();
            let size = bounds.size();
            let max_dim = size.x.max(size.y).max(size.z);

            self.camera.target = center;
            self.camera.distance = max_dim * 2.0;
        }
    }

    /// Tessellate all shapes and prepare meshes.
    pub fn prepare_meshes(&mut self, options: &TessellationOptions) {
        for obj in &mut self.objects {
            if let Some(ref shape) = obj.shape {
                obj.mesh = tessellate_with_options(shape, options);
                obj.mesh.transform(shape.transform());
            }
        }
    }
}

/// An object in the 3D scene.
#[derive(Debug, Clone)]
pub struct SceneObject {
    /// Optional name.
    pub name: Option<String>,
    /// The shape (for tessellation).
    pub shape: Option<Shape>,
    /// The tessellated mesh.
    pub mesh: Mesh,
    /// The material.
    pub material: Material,
    /// Transform matrix.
    pub transform: Mat4,
}

impl SceneObject {
    /// Create an object from a shape.
    pub fn from_shape(shape: Shape) -> Self {
        let mesh = tessellate(&shape, 0.1);
        Self {
            name: None,
            shape: Some(shape),
            mesh,
            material: Material::default(),
            transform: Mat4::IDENTITY,
        }
    }

    /// Create an object from a mesh.
    pub fn from_mesh(mesh: Mesh) -> Self {
        Self {
            name: None,
            shape: None,
            mesh,
            material: Material::default(),
            transform: Mat4::IDENTITY,
        }
    }

    /// Set the name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the material.
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// Apply a transform.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }
}

/// Camera settings.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Target point the camera is looking at.
    pub target: Vec3,
    /// Distance from target.
    pub distance: f32,
    /// Azimuth angle (horizontal rotation).
    pub azimuth: f32,
    /// Elevation angle (vertical rotation).
    pub elevation: f32,
    /// Field of view in degrees.
    pub fov: f32,
    /// Near clipping plane.
    pub near: f32,
    /// Far clipping plane.
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 10.0,
            azimuth: 45.0_f32.to_radians(),
            elevation: 30.0_f32.to_radians(),
            fov: 45.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    /// Get the camera position.
    pub fn position(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.sin();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * self.elevation.cos() * self.azimuth.cos();
        self.target + Vec3::new(x, y, z)
    }

    /// Get the view matrix.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position(), self.target, Vec3::Y)
    }

    /// Get the projection matrix for the given aspect ratio.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov.to_radians(), aspect, self.near, self.far)
    }

    /// Orbit the camera around the target.
    pub fn orbit(&mut self, delta_azimuth: f32, delta_elevation: f32) {
        self.azimuth += delta_azimuth;
        self.elevation = (self.elevation + delta_elevation)
            .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
    }

    /// Zoom the camera (change distance).
    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).max(0.1);
    }
}

/// Build a 3D scene from a document.
pub fn build_scene(doc: &Document) -> Scene3D {
    let mut scene = Scene3D::new();

    for element in &doc.elements {
        if let Element::Part(part) = element {
            if let Some(obj) = build_part_object(part) {
                scene.add_object(obj);
            }
        }
    }

    scene.fit_camera();
    scene
}

fn build_part_object(part: &PartElement) -> Option<SceneObject> {
    let shape = build_geometry(&part.geometry)?;
    let material = extract_material(&part.properties);

    let mut obj = SceneObject::from_shape(shape)
        .with_material(material);

    if let Some(ref name) = part.name {
        obj = obj.with_name(name.0.as_str());
    }

    Some(obj)
}

fn build_geometry(geometry: &seed_core::ast::Geometry) -> Option<Shape> {
    use seed_core::ast::Geometry;

    match geometry {
        Geometry::Primitive(prim) => build_primitive(prim),
        Geometry::Csg(op) => build_csg(op),
    }
}

fn build_primitive(prim: &seed_core::ast::Primitive) -> Option<Shape> {
    use seed_core::ast::Primitive;

    match prim {
        Primitive::Box { width, height, depth } => {
            let w = width.to_mm().unwrap_or(10.0);
            let h = height.to_mm().unwrap_or(10.0);
            let d = depth.to_mm().unwrap_or(10.0);
            Some(Shape::box_shape(w, h, d))
        }
        Primitive::Cylinder { radius, height } => {
            let r = radius.to_mm().unwrap_or(5.0);
            let h = height.to_mm().unwrap_or(10.0);
            Some(Shape::cylinder(r, h))
        }
        Primitive::Sphere { radius } => {
            let r = radius.to_mm().unwrap_or(5.0);
            Some(Shape::sphere(r))
        }
    }
}

fn build_csg(op: &seed_core::ast::CsgOperation) -> Option<Shape> {
    use seed_core::ast::CsgOperation;

    match op {
        CsgOperation::Union(geometries) => {
            let shapes: Vec<Shape> = geometries.iter()
                .filter_map(build_geometry)
                .collect();
            if shapes.is_empty() {
                None
            } else {
                Some(Shape::compound(shapes))
            }
        }
        CsgOperation::Difference { base, subtract } => {
            let mut result = build_geometry(base)?;
            for sub in subtract {
                if let Some(sub_shape) = build_geometry(sub) {
                    result = result.difference(&sub_shape);
                }
            }
            Some(result)
        }
        CsgOperation::Intersection(geometries) => {
            let mut shapes = geometries.iter().filter_map(build_geometry);
            let first = shapes.next()?;
            Some(shapes.fold(first, |acc, s| acc.intersection(&s)))
        }
    }
}

fn extract_material(properties: &[Property]) -> Material {
    let mut material = Material::default();

    for prop in properties {
        match prop.name.as_str() {
            "color" | "fill" => {
                if let PropertyValue::Color(c) = &prop.value {
                    material.color = *c;
                }
            }
            "metallic" => {
                if let PropertyValue::Number(n) = &prop.value {
                    material.metallic = (*n as f32).clamp(0.0, 1.0);
                }
            }
            "roughness" => {
                if let PropertyValue::Number(n) = &prop.value {
                    material.roughness = (*n as f32).clamp(0.0, 1.0);
                }
            }
            _ => {}
        }
    }

    material
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_new() {
        let scene = Scene3D::new();
        assert!(scene.objects.is_empty());
        assert!(!scene.lights.is_empty());
    }

    #[test]
    fn test_camera_position() {
        let camera = Camera::default();
        let pos = camera.position();

        // Camera should be above and in front of target
        assert!(pos.y > 0.0);
        assert!(pos.length() > 0.0);
    }

    #[test]
    fn test_camera_orbit() {
        let mut camera = Camera::default();
        let initial_pos = camera.position();

        camera.orbit(0.5, 0.0);
        let new_pos = camera.position();

        // Position should change after orbit
        assert!((initial_pos - new_pos).length() > 0.1);
    }

    #[test]
    fn test_scene_object_from_shape() {
        let shape = Shape::box_shape(10.0, 10.0, 10.0);
        let obj = SceneObject::from_shape(shape);

        assert!(obj.mesh.triangle_count() > 0);
    }

    #[test]
    fn test_scene_fit_camera() {
        let mut scene = Scene3D::new();
        scene.add_object(SceneObject::from_shape(Shape::box_shape(100.0, 100.0, 100.0)));
        scene.fit_camera();

        // Camera should be far enough to see the large object
        assert!(scene.camera.distance > 100.0);
    }
}
