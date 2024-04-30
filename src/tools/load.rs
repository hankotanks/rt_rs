use std::{fs, io, marker};

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
#[clap(group(
    clap::ArgGroup::new("handler")
        .args(&["handler-bvh", "handler-blank"])
        .multiple(false)
))]
struct Args {
    // The path to the desired scene (JSON)
    #[clap(long, value_parser, default_value_t = String::from("scenes/default.json"))]
    path: String,

    #[clap(long = "handler-blank", action)]
    handler_blank: bool,

    #[clap(long = "handler-bvh", action)]
    handler_bvh: bool,

    #[clap(long = "data", value_parser)]
    path_data: Option<String>,

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

    #[clap(long = "ambience", value_parser)]
    compute_ambience: Option<f32>,
}

fn start<H: rt::handlers::IntrsHandler>(
    resolution: rt::Resolution, 
    fps: Option<u32>,
    compute: rt::ComputeConfig, 
    scene: rt::scene::Scene,
) -> anyhow::Result<()> {
    let config_default = rt::Config::<H>::default();
    let config: rt::Config<H> = rt::Config {
        resolution,
        compute,
        fps: fps.unwrap_or(config_default.fps),
        ..Default::default()
    };

    pollster::block_on({
        rt::run_native(config, scene)
    })
}

fn main() -> anyhow::Result<()> {
    use clap::Parser as _;

    let args = Args::parse();

    let Args {
        path,
        handler_blank,
        handler_bvh,
        path_data,
        width,
        height,
        workgroup_size,
        fps,
        compute_bounces,
        compute_camera_light_source,
        compute_ambience, ..
    } = args;

    let resolution =  match (width, height, workgroup_size) {
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

    let scene_reader = io::BufReader::new({
        fs::File::open(path)?
    });

    let scene: rt::scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    if handler_blank {
        start::<rt::handlers::BlankIntrs>(resolution, fps, compute, scene)
    } else if handler_bvh {
        use io::Read as _;

        if let Some(path_data) = path_data {
            let bytes: Result<Vec<u8>, io::Error> = fs::File::open(path_data)?
                .bytes()
                .collect();

            rt::handlers::BvhIntrs::prepare(bytes?.as_slice())?;
        }

        start::<rt::handlers::BvhIntrs>(resolution, fps, compute, scene)
    } else {
        start::<rt::handlers::BasicIntrs>(resolution, fps, compute, scene)
    }
}