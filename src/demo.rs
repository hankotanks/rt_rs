use std::{fs, io};

fn main() -> anyhow::Result<()> {
    let config = tracer::Config {
        resolution: tracer::Resolution::Dynamic(16),
        fps: 60,
        ..Default::default()
    };

    let scene_reader = io::BufReader::new({
        fs::File::open("scenes/default.json")?
    });
    
    let scene: tracer::scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    pollster::block_on(tracer::run_native(config, scene))
}