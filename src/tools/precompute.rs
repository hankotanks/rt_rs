use std::{io, fs, path};
use std::io::Write as _;

fn main() -> anyhow::Result<()> {
    let parsed = clap::Command::new(env!("CARGO_BIN_NAME"))
        .arg(
            clap::Arg::new("out")
                .long("out")
                .number_of_values(1)
                .required(true))
        .arg(
            clap::Arg::new("scene")
                .long("scene")
                .number_of_values(1)
                .required(true))
        .arg(
            clap::Arg::new("eps")
                .long("eps")
                .number_of_values(1)
                .value_parser(clap::value_parser!(f32)))
        .get_matches();


    let out = parsed
        .get_one::<String>("out")
        .map(|temp| path::PathBuf::from(temp))
        .unwrap();

    let scene_reader = parsed
        .get_one::<String>("scene")
        .map(path::PathBuf::from)
        .map(fs::File::open)
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?
        .map(io::BufReader::new)
        .unwrap();

    let config = match parsed.get_one::<f32>("eps") {
        Some(eps) => rt::handlers::BvhConfig { eps: *eps, },
        None => rt::handlers::BvhConfig::default(),
    };

    let scene = serde_json::from_reader(scene_reader)?;

    let bvh = rt::bvh::BvhData::new({
        &rt::bvh::Aabb::from_scene(config.eps, &scene)?
    });
    
    fs::File::create(out)?
        .write(serde_json::to_string(&bvh)?.as_bytes())?;

    Ok(())
}