use std::{fs, io};

fn main() -> anyhow::Result<()> {
    let scene_reader = io::BufReader::new({
        fs::File::open("scenes/default.json")?
    });
    
    let scene: rt::scene::Scene = //
        serde_json::from_reader(scene_reader)?;

    println!("{:#?}", rt::handlers::Aabb::from_scene(0.000002, &scene)?);

    Ok(())
}