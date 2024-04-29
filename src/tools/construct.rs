use std::{fs, path};
use std::io::Write as _;

use rt::geom;
use rt::geom::light as light;

use rt::scene;

fn main() -> anyhow::Result<()> {
    let parsed = clap::Command::new(env!("CARGO_BIN_NAME"))
        .arg(
            clap::Arg::new("out")
                .long("out")
                .number_of_values(1)
                .required(true))
        .arg(
            clap::Arg::new("light")
                .long("light")
                .number_of_values(4)
                .value_parser(clap::value_parser!(f32))
                .action(clap::ArgAction::Append))
        .arg(
            clap::Arg::new("model")
                .long("model")
                .required(true)
                .min_values(1)
                .action(clap::ArgAction::Append))
        .arg(
            clap::Arg::new("camera-pos")
                .long("camera-pos")
                .number_of_values(6)
                .value_parser(clap::value_parser!(f32))
                .required(true))
        .arg(
            clap::Arg::new("camera-fixed")
                .long("camera-fixed")
                .conflicts_with("camera-orbit")
                .action(clap::ArgAction::SetTrue))
        .arg(
            clap::Arg::new("camera-orbit")
                .long("camera-orbit")
                .conflicts_with("camera-fixed")
                .action(clap::ArgAction::SetTrue))
        .arg(
            clap::Arg::new("material")
                .long("material")
                .number_of_values(7)
                .value_parser(clap::value_parser!(f32))
                .action(clap::ArgAction::Append))
        .get_matches();

    let mut lights = parsed
        .get_many::<f32>("light")
        .unwrap_or_default()
        .copied()
        .collect::<Vec<_>>()
        .as_slice()
        .chunks_exact(4)
        .map(|values| {
            let [x, y, z, strength] = values else {
                anyhow::bail!("Flag --light expects 4 float values");
            };

            Ok(geom::light::Light { 
                pos: [*x, *y, *z], 
                strength: *strength,
            })
        }).collect::<Result<Vec<_>, anyhow::Error>>()?;

    if lights.is_empty() {
        let dummy = light::Light {
            pos: [0.; 3],
            strength: 0.,
        };

        lights.push(dummy);
    }

    let mut materials = parsed
        .get_many::<f32>("material")
        .unwrap_or_default()
        .copied()
        .collect::<Vec<_>>()
        .as_slice()
        .chunks(7)
        .map(|values| {
            let [r, g, b, a0, a1, a2, spec] = values else {
                anyhow::bail!("Flag --material expects 7 float values");
            };

            Ok(geom::PrimMat::new(
                [*r, *g, *b],
                [*a0, *a1, *a2],
                *spec
            ))
        }).collect::<Result<Vec<_>, anyhow::Error>>()?;

    let models = parsed
        .get_many::<String>("model")
        .unwrap_or_default()
        .collect::<Vec<_>>()
        .as_slice()
        .chunks_exact(2)
        .map(|data| {
            let [model, material] = data else {
                anyhow::bail!("\
                    Flag --model expects 2 arguments:
                        [0] Path to OBJ file
                        [1] Material index to apply (or 'default')\
                ");
            };

            let material = if material.contains("default") {
                None
            } else if let Ok(idx) = material.parse::<u32>() {
                Some(idx)
            } else {
                anyhow::bail!("\
                    Flag --model expects 2 arguments:
                        [0] Path to OBJ file
                        [1] Material index to apply (or 'default')\
                ");
            };

            Ok((path::PathBuf::from(model), material))
        }).collect::<Result<Vec<_>, anyhow::Error>>()?;

    if materials.is_empty() || models.iter().any(|(_, idx)| idx.is_none()) {
        let red = geom::PrimMat::new(
            [0.5, 0.1, 0.1],
            [0.9, 0.1, 0.],
            10.,
        ); 

        materials.insert(0, red);
    }

    if models.is_empty() {
        anyhow::bail!("At least one model must be provided");
    }
    
    let camera = {
        let values = parsed
            .get_many::<f32>("camera-pos")
            .unwrap_or_default()
            .copied()
            .collect::<Vec<_>>();

        let [p0, p1, p2, a0, a1, a2] = values[..] else {
            anyhow::bail!("Flag --camera-pos expects 6 float values");
        };

        scene::CameraUniform::new([p0, p1, p2], [a0, a1, a2])
    };

    let camera_controller = if *parsed.get_one::<bool>("camera-fixed").unwrap() {
        scene::CameraController::Fixed
    } else if *parsed.get_one::<bool>("camera-orbit").unwrap() {
        scene::CameraController::Orbit { left: false, right: false, }
    } else {
        anyhow::bail!("Camera controller must be specified");
    };

    let mut scene = scene::Scene::Active {
        camera,
        camera_controller,
        prims: Vec::new(),
        vertices: Vec::new(),
        lights,
        materials,
    };

    for (path, idx) in models {
        let obj = wavefront::Obj::from_file(path)?;

        let idx = match idx {
            Some(idx) => (idx + 1) as i32,
            None => 0,
        };

        scene.add_mesh(obj, idx)?;
    }

    let out = parsed
        .get_one::<String>("out")
        .map(|temp| path::PathBuf::from(temp))
        .unwrap();

    fs::File::create(out)?
        .write(serde_json::to_string_pretty(&scene)?.as_bytes())?;

    Ok(())
}