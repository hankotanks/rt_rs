use std::{fs, io, env};

use rt::handlers;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        anyhow::bail!({
            io::Error::new(
                io::ErrorKind::InvalidInput, 
                "Expected a single argument \
                (the path to a JSON-specified scene)."
            )
        });
    }

    let config = rt::Config {
        resolution: rt::Resolution::Dynamic(16),
        fps: 60,
        ..Default::default()
    };

    let scene_reader = io::BufReader::new({
        fs::File::open(&args[1])?
    });

    let scene: rt::scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    pollster::block_on(rt::run_native::<handlers::BasicIntrs>(config, scene))
}