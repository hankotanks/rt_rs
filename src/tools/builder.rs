use std::{fs, path};
use std::io::Write as _;

use rt::{geom::{self, light}, scene};

fn main() -> anyhow::Result<()> {
    let cmd = clap::Command::new(env!("CARGO_BIN_NAME"))
        .arg(
            clap::Arg::new("light")
                .long("light")
                .number_of_values(4)
                .value_parser(clap::value_parser!(f32))
                .action(clap::ArgAction::Append))
        .arg(
            clap::Arg::new("models")
                .long("models")
                .min_values(1))
        .get_matches();

    let lights = cmd
        .get_many::<f32>("light")
        .unwrap_or_default()
        .copied()
        .collect::<Vec<_>>()
        .as_slice()
        .chunks_exact(4)
        .map(|values| {
            let [x, y, z, strength] = values else { panic!(); };

            rt::geom::light::Light { 
                pos: [*x, *y, *z], 
                strength: *strength,
            }
        }).collect::<Vec<_>>();

    let models = cmd
        .values_of("models")
        .unwrap_or_default()
        .map(|model| path::PathBuf::from(model))
        .map(|model_path| wavefront::Obj::from_file(model_path))
        .collect::<Result<Vec<_>, wavefront::Error>>()?;

    /*
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
        .write(scene_serialized.as_bytes())?;*/

    Ok(())
}