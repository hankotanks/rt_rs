use std::fs;
use std::io::Write as _;

use rt::{scene, geom};

fn main() -> anyhow::Result<()> {
    let mut scene = scene::Scene::Active {
        camera: scene::camera::CameraUniform::new([0., 0., -30.], [0.; 3]),
        camera_controller: scene::camera::CameraController::Orbit { left: false, right: false, scroll: 0 },
        prims: vec![],
        vertices: vec![],
        lights: vec![
            geom::light::Light { pos: [0., 30., 0.], strength: 2. }
        ],
        materials: vec![
            geom::PrimMat::new([0.7, 0.2, 0.3], [0.9, 0.1, 0.], 50.)
        ],
    };

    scene.add_mesh(wavefront::Obj::from_file("meshes/shuttle.obj")?, 0)?;

    let scene_serialized = serde_json::to_string_pretty(&scene)?;

    fs::File::create("scenes/test.json")?
        .write(scene_serialized.as_bytes())?;

    Ok(())
}