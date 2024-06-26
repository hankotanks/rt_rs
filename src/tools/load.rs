use std::{fs, io};

use winit::dpi;

use rt::{handlers, timing, scene};

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
        .args(&["handler-bvh", "handler-bvh-rf", "handler-naive"])
        .multiple(false)
))]
struct Args {
    // The path to the desired scene (JSON)
    #[clap(long, value_parser, default_value_t = String::from("scenes/default.json"))]
    path: String,

    #[clap(long = "handler-naive", action)]
    handler_naive: bool,

    // This argument takes a single value
    // Either the epsilon used to construct the BVH
    // Or a path to a preconstructed BVH
    #[clap(long = "handler-bvh", value_parser, min_values = 0, max_values = 1)]
    handler_bvh: Option<Vec<String>>,

    #[clap(long = "handler-bvh-rf", value_parser, min_values = 0, max_values = 1)]
    handler_bvh_rf: Option<Vec<f32>>,

    #[clap(long = "benchmark", action)]
    benchmark: bool,

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

fn start<H: handlers::IntrsHandler>(
    benchmark: bool,
    resolution: rt::Resolution, 
    fps: Option<u32>,
    config_compute: rt::ComputeConfig, 
    config_handler: H::Config,
    scene: scene::Scene,
) -> anyhow::Result<()> {
    let config_default = rt::Config::default();
    let config: rt::Config = rt::Config {
        resolution,
        compute: config_compute,
        fps: fps.unwrap_or(config_default.fps),
    };
    
    if benchmark {
        pollster::block_on({
            rt::run_native::<H, timing::BenchScheduler>
                (config, config_handler, scene)
        })
    } else {
        pollster::block_on({
            rt::run_native::<H, timing::DefaultScheduler>
                (config, config_handler, scene)
        })
    }
}

fn main() -> anyhow::Result<()> {
    use clap::Parser as _;

    let args = Args::parse();

    let Args {
        path,
        handler_naive,
        handler_bvh,
        handler_bvh_rf,
        benchmark,
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

    let config_compute_default = rt::ComputeConfig::default();
    let config_compute = rt::ComputeConfig {
        bounces: compute_bounces
            .unwrap_or(config_compute_default.bounces),
        camera_light_source: compute_camera_light_source
            .unwrap_or(config_compute_default.camera_light_source),
        ambience: compute_ambience
            .unwrap_or(config_compute_default.ambience),
        ..Default::default()
    };

    let scene_reader = io::BufReader::new({
        fs::File::open(path)?
    });

    let scene: scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    if handler_naive {
        start::<handlers::BasicIntrs>
            (benchmark, resolution, fps, config_compute, (), scene)
    } else if let Some(args) = handler_bvh {
        use io::Read as _;

        let config_handler: handlers::BvhConfig = match args.len() {
            0 => handlers::BvhConfig::Default,
            1 => {
                match args[0].parse::<f32>() {
                    Ok(eps) => handlers::BvhConfig::Runtime { eps, },
                    Err(_) => match fs::File::open(&args[0]) {
                        Ok(file) => {
                            let bytes = file
                                .bytes()
                                .collect::<Result<Vec<_>, io::Error>>()?;

                            handlers::BvhConfig::Bytes(bytes)
                        },
                        Err(_) => anyhow::bail!("\
                            Flag --handler-bvh requires either:
                              - The path to a precomputed BVH file
                              - An epsilon value (f32)\
                        "),
                    },
                }
            },
            _ => unreachable!(),
        };

        start::<handlers::BvhIntrs>
            (benchmark, resolution, fps, config_compute, config_handler, scene)
    } else if let Some(args) = handler_bvh_rf {
        let config_handler = match args.len() {
            0 => handlers::RfBvhConfig::default(),
            1 => handlers::RfBvhConfig::Eps(args[0]),
            _ => unreachable!(),
        };

        start::<handlers::RfBvhIntrs>
            (benchmark, resolution, fps, config_compute, config_handler, scene)
    } else {
        start::<handlers::BlankIntrs>
            (benchmark, resolution, fps, config_compute, (), scene)
    }
}