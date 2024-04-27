use std::{fs, io};

use rt::handlers;

fn main() -> anyhow::Result<()> {
    let config = rt::Config {
        resolution: rt::Resolution::Dynamic(16),
        fps: 60,
        ..Default::default()
    };

    let scene_reader = io::BufReader::new({
        fs::File::open("scenes/default.json")?
    });
    
    let scene: rt::scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    pollster::block_on(rt::run_native::<handlers::BvhIntrs>(config, scene))
}