use std::{io, fs, path};

use rt::{bvh, handlers};

fn main() -> anyhow::Result<()> {
    use std::io::Write as _;

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
        .arg(
            clap::Arg::new("item-count")
                .long("item-count")
                .number_of_values(1)
                .value_parser(clap::value_parser!(usize))
                .required(true))
        .get_matches();

    let out = parsed
        .get_one::<String>("out")
        .map(path::PathBuf::from)
        .unwrap();

    let scene_reader = parsed
        .get_one::<String>("scene")
        .map(path::PathBuf::from)
        .map(fs::File::open)
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?
        .map(io::BufReader::new)
        .unwrap();

    let scene = serde_json::from_reader(scene_reader)?;

    let eps = match parsed.get_one::<f32>("eps") {
        Some(eps) => *eps,
        None => handlers::BvhIntrs::default().eps,
    };

    // This is required so we can safely unwrap
    let item_count = parsed
        .get_one::<>("item-count")
        .unwrap();

    let bvh = rt::bvh::BvhData::new({
        &bvh::Aabb::from_scene(eps, &scene, *item_count)
    });
    
    fs::File::create(out)?
        .write_all(serde_json::to_string(&bvh)?.as_bytes())?;

    Ok(())
}