use std::{fs, io};

use winit::dpi;

#[derive(clap::Parser)]
#[derive(Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(group(
    clap::ArgGroup::new("resolution")
        .args(&["width", "height", "workgroup-size"])
        .required(true)
        .multiple(true)
))]
#[clap(group(
    clap::ArgGroup::new("size")
        .args(&["width", "height"])
        .requires_all(&["width", "height"])
        .multiple(true)
))]
struct Args {
    // The path to the desired scene (JSON)
    #[clap(long, value_parser, default_value_t = String::from("scenes/default.json"))]
    path: String,

    #[clap(long, short, value_parser)]
    width: Option<u32>,

    #[clap(long, short, value_parser)]
    height: Option<u32>,

    #[clap(long = "workgroup-size", value_parser)]
    workgroup_size: Option<u32>,

    #[clap(long, value_parser)]
    fps: Option<u32>,

    #[clap(long = "bounces", value_parser)]
    compute_bounces: Option<u32>,

    #[clap(long = "camera-light-strength", value_parser)]
    compute_camera_light_source: Option<f32>,

    #[clap(long = "ambience-strength", value_parser)]
    compute_ambience: Option<f32>,
}

fn main() -> anyhow::Result<()> {
    use clap::Parser as _;

    let Args {
        path,
        width,
        height,
        workgroup_size: wg,
        fps,
        compute_bounces,
        compute_camera_light_source,
        compute_ambience, ..
    } = Args::parse();

    let resolution =  match (width, height, wg) {
        (None, None, Some(wg)) => //
            rt::Resolution::Dynamic(wg),
        (Some(width), Some(height), None) => //
            rt::Resolution::Sized(dpi::PhysicalSize::new(width, height)),
        (Some(width), Some(height), Some(wg)) => //
            rt::Resolution::Fixed { 
                size: dpi::PhysicalSize::new(width, height), 
                wg,
            },
        _ => rt::Resolution::default(),
    };

    let compute_default = rt::ComputeConfig::default();
    let compute = rt::ComputeConfig {
        bounces: compute_bounces
            .unwrap_or(compute_default.bounces),
        camera_light_source: compute_camera_light_source
            .unwrap_or(compute_default.camera_light_source),
        ambience: compute_ambience
            .unwrap_or(compute_default.ambience),
        ..Default::default()
    };

    let config_default = rt::Config::default();
    let config = rt::Config {
        resolution,
        compute,
        fps: fps.unwrap_or(config_default.fps),
        ..Default::default()
    };
    
    let scene_reader = io::BufReader::new({
        fs::File::open(path)?
    });

    let scene: rt::scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    pollster::block_on({
        rt::run_native::<rt::handlers::BasicIntrs>(config, scene)
    })
}