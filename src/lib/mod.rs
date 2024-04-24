pub mod geom;
pub mod pipelines;
pub mod scene;
pub mod vertex;
pub mod state;
pub mod handlers;
pub mod shaders;

#[cfg(target_arch = "wasm32")]
mod web;

use std::sync;

use winit::{dpi, event, event_loop, keyboard, window};

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// Define the error type based on the platform
cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use wasm_bindgen::prelude::*;

        type Failed = JsValue;
    } else {
        type Failed = anyhow::Error;
    }
}

// This is a wrapper function to avoid having to cast Err variants
#[allow(non_snake_case)]
fn BAIL<T, E: Into<anyhow::Error>>(result: Result<T, E>) -> Result<T, Failed> {
    #[allow(clippy::let_and_return)]
    result.map_err(|e| {
        let e = Into::<anyhow::Error>::into(e);

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                JsValue::from_str(&format!("{}", e))
            } else {
                e
            }
        }
    })
}

// The target texture resolution
#[derive(Clone, Copy)]
#[derive(serde::Deserialize)]
#[serde(untagged)]
pub enum Resolution {
    Dynamic(u32),
    Sized(dpi::PhysicalSize<u32>),
    Fixed { 
        size: dpi::PhysicalSize<u32>, 
        wg: u32 
    }
}

impl Resolution {
    pub fn wg(&self) -> u32 {
        // dim = GCF(width, height);
        let dim = match self {
            Resolution::Dynamic(wg) => *wg,
            Resolution::Sized(size) => {
                let dpi::PhysicalSize {
                    mut width,
                    mut height,
                } = size;

                while height != 0 {
                    let temp = width;

                    width = height;
                    height = temp % height;
                }
                
                width
            },
            Resolution::Fixed { wg, .. } => *wg,
        };

        // Hard limit the local group size
        // WebGPU only supports 256 instances per workgroup at maximum
        if dim * dim > 256 { 16 } else { dim }
    }
}

// These config options will be passed to the compute shader
#[repr(C)]
#[derive(Clone, Copy, Default)]
#[derive(serde::Deserialize)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[serde(default)]
pub struct ComputeConfig {
    pub t_min: f32,
    pub t_max: f32,
    pub camera_light_source: f32,
    pub bounces: u32,
    pub eps: f32,
    pub ambience: f32,
}

impl ComputeConfig {
    const fn new() -> Self {
        Self {
            t_min: 0.01,
            t_max: 1000.,
            camera_light_source: 1.,
            bounces: 4,
            eps: 0.0000001,
            ambience: 0.2,
        }
    }
}

// Config declaration
#[derive(Clone, Copy)]
#[derive(serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub compute: ComputeConfig,
    pub resolution: Resolution,
    pub fps: u32,
    pub canvas_raw_handle: u32,
    pub updated: bool,
}

impl Default for Config {
    fn default() -> Self { Self::new() }
}

// Config::update defaults to true, so deserialization automatically
// sets the flag
impl Config {
    const fn new() -> Self {
        Self { 
            compute: ComputeConfig::new(),
            resolution: Resolution::Sized(dpi::PhysicalSize::new(640, 480)),
            fps: 60,
            canvas_raw_handle: 2024,
            updated: true,
        }
    }
}

// The Config global that the JS interface writes into
pub static mut CONFIG: Config = Config::new();

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn update_config(serialized: JsValue) -> Result<(), Failed> {
    use std::io;

    match serialized.as_string() {
        Some(temp) => unsafe {
            CONFIG = BAIL(serde_json::from_str::<Config>(&temp))?;
        },
        None => {
            return BAIL(Err(io::Error::from(io::ErrorKind::InvalidData)));
        },
    }

    Ok(())
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub async fn run() -> Result<(), Failed> {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            console_error_panic_hook::set_once();

            wasm_logger::init(wasm_logger::Config::default());
        } else {
            simple_logger::SimpleLogger::new()
                .with_level(log::LevelFilter::Info)
                .init()
                .unwrap();
        }
    }    

    // TODO
    let mut scene = scene::Scene {
        camera: scene::camera::CameraUniform::new(
            [0., 0., -10.], 
            [0.; 3]
        ),
        camera_controller: scene::camera::CameraController::Orbit { 
            left: false, 
            right: false, 
            scroll: 0 
        },
        prims: vec![],
        vertices: vec![],
        lights: vec![
            geom::light::Light { pos: [-20., 20., 20.], strength: 1.5, },
            geom::light::Light { pos: [30., 50., -25.], strength: 1.8, },
            geom::light::Light { pos: [30., 20., 30.], strength: 1.7, },
        ],
        materials: vec![
            geom::PrimMat::new(
                [0.4, 0.4, 0.3],
                [0.6, 0.3, 0.1],
                50.,
            ),
            geom::PrimMat::new(
                [0.3, 0.1, 0.1],
                [0.9, 0.1, 0.],
                 10.,
            ),
            geom::PrimMat::new(
                [1.; 3],
                [0., 10., 0.8],
                1425.,
            )
        ],
    };

    let mesh = include_bytes!("../../meshes/tetrahedron.obj");
    let mesh = BAIL(wavefront::Obj::from_reader(&mesh[..]))?;

    scene.add_mesh(mesh, 1);

    let mesh = include_bytes!("../../meshes/dodecahedron.obj");
    let mesh = BAIL(wavefront::Obj::from_reader(&mesh[..]))?;

    scene.add_mesh(mesh, 0);
    
    let event_loop = BAIL(event_loop::EventLoop::new())?;
        event_loop.set_control_flow(event_loop::ControlFlow::Poll);

    let window = BAIL({
        window::WindowBuilder::new().build(&event_loop)
    })?;

    // Initialize the canvas
    #[cfg(target_arch = "wasm32")] unsafe {
        web::init(CONFIG, &window);
    }

    // This needs to be shared with State
    let window = sync::Arc::new(window);

    // Initialize the state (bail on failure)
    let mut state = unsafe {
        use handlers::basic::BasicIntrs;

        let window = window.clone();
        BAIL({
            state::State::<BasicIntrs>::new(CONFIG, &scene, window).await
        })?
    };

    // Keeps track of resize actions. 
    // We want to wait until the user is done resizing to
    // avoid repeatedly resizing the texture
    let mut resize_instant = chrono::Local::now();
    let mut resize_dim = None;

    let mut curr_frame_instant = chrono::Local::now();
    let mut curr_frame_duration = 0.;

    // This keep track of failures
    // if failure.is_err(), target.exit()
    let mut failure = Ok(());

    // Enter the event loop
    BAIL(event_loop.run(|event, target| {
        // Take a snapshot of the current Instant
        let frame_instant = chrono::Local::now();
        let frame_duration = unsafe { (CONFIG.fps as f64).recip() * 1_000. };

        let temp = curr_frame_instant.signed_duration_since(frame_instant);
        let temp = temp
            .num_microseconds()
            .map(|micros| 0.001 * micros as f64)
            .unwrap_or(temp.num_milliseconds() as f64)
            .abs();

        curr_frame_instant = frame_instant;
        curr_frame_duration += temp;

        unsafe {
            if CONFIG.updated {
                CONFIG.updated = false;
                
                state.update_config(CONFIG.compute);
            }
        }

        match event {
            event::Event::WindowEvent { event, window_id, .. }
                if window_id == window.id() => {
                if !scene.camera_controller.handle_event(&event) {
                    match event {
                        event::WindowEvent::CloseRequested | //
                        event::WindowEvent::KeyboardInput {
                            event: event::KeyEvent {
                                state: event::ElementState::Pressed,
                                logical_key: keyboard::Key::Named(keyboard::NamedKey::Escape), ..
                            }, ..
                        } => target.exit(),
                        event::WindowEvent::Resized(physical_size) //
                            if resize_dim != Some(physical_size) => {
                            
                            // Update the size and the time the event occurred
                            // This will later be used to avoid excess resize actions
                            resize_dim = Some(physical_size);
                            resize_instant = chrono::Local::now();
                        },
                        event::WindowEvent::RedrawRequested => {
                            match state.render() {
                                Ok(_) => { /*  */ },
                                Err(wgpu::SurfaceError::Lost | 
                                    wgpu::SurfaceError::Outdated
                                ) => unsafe {
                                    state.resize(CONFIG, state.window_size());
                                },
                                Err(e) => failure = BAIL(Err(e)),
                            }
                        },
                        _ => { /*  */ },
                    }
                }},
            _ => { /*  */ },
        }

        // Indicates whether the camera has changed
        let mut update_required_camera = false;

        let scene::Scene { 
            camera, 
            camera_controller, ..
        } = &mut scene;

        // Update the camera
        // NOTE: Camera updates are tied to FPS
        #[allow(clippy::collapsible_if)]
        if curr_frame_duration >= frame_duration {
            if camera_controller.update(camera) {
                state.update_camera_buffer(*camera);

                update_required_camera = true;
            }
        }

        // Calculate time since last resize event
        let resize_duration = resize_instant.signed_duration_since(frame_instant);
        let resize_duration = resize_duration
            .num_microseconds()
            .map(|micros| 0.001 * micros as f64)
            .unwrap_or(resize_duration.num_milliseconds() as f64)
            .abs();

        // Indicates that its time for the next frame
        let mut update_required_framerate = false;

        // If the user is done resizing, adjust texture and uniforms
        if resize_duration > frame_duration {
            if let Some(dim) = resize_dim.take() {
                unsafe { state.resize(CONFIG, dim); }

                // We want to begin an update immediately after resizing
                // update_required_framerate is co-opted for this purpose
                update_required_framerate = true;
            }
        }

        // Check if it's time for an update
        if curr_frame_duration >= frame_duration {
            curr_frame_duration -= frame_duration;

            update_required_framerate = true;
        }

        // Perform the update only if the camera and FPS call for it
        if update_required_framerate && update_required_camera {
            unsafe { state.update(CONFIG); }

            window.request_redraw();
        }

        // If we've ran into an error, start the process of exiting
        if failure.is_err() { target.exit(); }
    }))?;

    failure
}
