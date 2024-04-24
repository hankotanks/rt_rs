fn main() -> anyhow::Result<()> {
    let config = tracer::Config {
        resolution: tracer::Resolution::Dynamic(16),
        fps: 60,
        ..Default::default()
    };
    
    let mut scene = tracer::scene::Scene {
        camera: tracer::scene::camera::CameraUniform::new(
            [0., 0., -10.], 
            [0.; 3]
        ),
        camera_controller: tracer::scene::camera::CameraController::Orbit { 
            left: false, 
            right: false, 
            scroll: 0 
        },
        prims: vec![],
        vertices: vec![],
        lights: vec![
            tracer::geom::light::Light { pos: [-20., 20., 20.], strength: 1.5, },
            tracer::geom::light::Light { pos: [30., 50., -25.], strength: 1.8, },
            tracer::geom::light::Light { pos: [30., 20., 30.], strength: 1.7, },
        ],
        materials: vec![
            tracer::geom::PrimMat::new(
                [0.4, 0.4, 0.3],
                [0.6, 0.3, 0.1],
                50.,
            ),
            tracer::geom::PrimMat::new(
                [0.3, 0.1, 0.1],
                [0.9, 0.1, 0.],
                 10.,
            ),
            tracer::geom::PrimMat::new(
                [1.; 3],
                [0., 10., 0.8],
                1425.,
            )
        ],
    };

    let mesh = include_bytes!("../meshes/tetrahedron.obj");
    let mesh = wavefront::Obj::from_reader(&mesh[..])?;

    scene.add_mesh(mesh, 1);

    let mesh = include_bytes!("../meshes/dodecahedron.obj");
    let mesh = wavefront::Obj::from_reader(&mesh[..])?;

    scene.add_mesh(mesh, 0);

    pollster::block_on(tracer::run_native(config, scene))
}