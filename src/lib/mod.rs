mod pipelines;
mod vertex;
mod state;
mod shaders;

pub mod timing;
pub mod scene;
pub mod geom;
pub mod handlers;
pub mod bvh;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::{update_config, update_scene, update_viewport};

use std::sync;

use winit::{dpi, event, event_loop, keyboard, window};

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// Define the error type based on the platform
cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        type Failed = wasm_bindgen::JsValue;
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
                let _ = web::note("\
                    Encountered a critical error. \
                    Check console for details\
                ");

                wasm_bindgen::JsValue::from_str(&format!("{}", e))
            } else { e }
        }
    })
}

// The target texture resolution
#[derive(Clone, Copy)]
#[derive(serde::Deserialize)]
#[derive(Debug)]
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
    const fn new() -> Self {
        Self::Dynamic(16)
    }
}

impl Default for Resolution {
    fn default() -> Self { Self::new() }
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
#[derive(Clone, Copy)]
#[derive(serde::Deserialize)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Debug)]
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
            camera_light_source: 0.0,
            bounces: 4,
            eps: 0.0000001,
            ambience: 0.1,
        }
    }
}

impl Default for ComputeConfig {
    fn default() -> Self { Self::new() }
}

// Config declaration
#[derive(Clone, Copy)]
#[derive(serde::Deserialize)]
#[derive(Debug)]
#[serde(default)]
pub struct Config {
    pub compute: ComputeConfig,
    pub resolution: Resolution,
    pub fps: u32,
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
            resolution: Resolution::new(),
            fps: 60,
        }
    }
}

#[allow(unused_mut)]
pub async fn run_native<H, S>(
    mut config: Config, 
    mut config_handler: H::Config,
    mut scene: scene::Scene
) -> Result<(), Failed> 
    where H: handlers::IntrsHandler, S: timing::Scheduler {

    unsafe {
        run_internal::<H, S>(&mut config, config_handler, &mut scene).await
    }
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn run_wasm() -> Result<(), Failed> {
    unsafe {
        #[allow(static_mut_refs)]
        let web::WebState {
            config,
            scene, ..
        } = &mut web::WEB_STATE;

        // We don't take benchmarks on WASM
        type WebScheduler = timing::DefaultScheduler;

        // TODO: I'm going to keep web::WebHandler == BasicIntrs
        // until optimizations are complete
        run_internal::<web::WebHandler, WebScheduler>
            (config, <web::WebHandler as handlers::IntrsHandler>::Config::default(), scene).await

            
    }
}

async unsafe fn run_internal<H, S>(
    config: &mut Config,
    config_handler: H::Config,
    scene: &mut scene::Scene
) -> Result<(), Failed> 
    where H: handlers::IntrsHandler, S: timing::Scheduler {

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
    
    let event_loop = BAIL(event_loop::EventLoop::new())?;
        event_loop.set_control_flow(event_loop::ControlFlow::Poll);

    let window = BAIL({
        window::WindowBuilder::new().build(&event_loop)
    })?;

    // Initialize the canvas (WASM only)
    #[cfg(target_arch = "wasm32")] BAIL(web::init(&window))?;

    // This needs to be shared with State
    let window = sync::Arc::new(window);

    // Initialize the state (bail on failure)
    let mut state = {
        let window = window.clone();

        BAIL(state::State::<S>::new::<H>(
            *config, config_handler, scene, window).await)?
    };

    // Keeps track of resize actions. 
    // We want to wait until the user is done resizing to
    // avoid repeatedly resizing the texture
    let mut resize_instant = chrono::Local::now();
    let mut resize_dim = None;

    let mut prev_frame_instant = chrono::Local::now();
    let mut prev_frame_duration = 0.;

    // This keep track of failures
    // if failure.is_err(), target.exit()
    let mut failure = Ok(());

    // Indicates whether the camera has changed
    let mut update_required_camera = false;

    // Enter the event loop
    BAIL(event_loop.run(|event, target| {
        // We are only updating config options live on the web
        // So it can be disabled on native
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] { 
                #[allow(unused_mut)]
                let mut update_required_web = unsafe {
                    web::update(&mut state)
                };
            } else { 
                let mut update_required_web = false;
            }
        }

        match event {
            event::Event::WindowEvent { event, window_id, .. }
                if window_id == window.id() => {

                let handled = match scene {
                    scene::Scene::Unloaded => false,
                    scene::Scene::Active { camera_controller, .. } => //
                        camera_controller.handle_event(&event),
                };

                if handled {
                    cfg_if::cfg_if! {
                        if #[cfg(target_arch = "wasm32")] {
                            let temp = true;
                        } else {
                            let temp = false;
                        }
                    }; update_required_web |= temp;
                } else {
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
                                ) => state.resize(*config, window.inner_size()),
                                Err(e) => failure = BAIL(Err(e)),
                            }
                        },
                        _ => { /*  */ },
                    }
                }},
            _ => { /*  */ },
        }

        // Take a snapshot of the current Instant
        let frame_instant = chrono::Local::now();
        let frame_duration = 1_000. * (config.fps as f64).recip();

        #[allow(unused_mut)]
        let mut temp = prev_frame_instant.signed_duration_since(frame_instant);
        let mut temp = temp
            .num_microseconds()
            .map(|micros| 0.001 * micros as f64)
            .unwrap_or(temp.num_milliseconds() as f64)
            .abs();

        // Prevent death spiral
        // https://cs.pomona.edu/classes/cs181g/notes/controlling-time.html
        if temp > frame_duration * 10. {
            temp = frame_duration; prev_frame_duration = 0.;
        }

        // Update the camera
        // NOTE: Camera updates are tied to FPS
        if let scene::Scene::Active { 
            camera, 
            camera_controller, .. 
        } = scene {
            if camera_controller.update(camera, temp as f32) {
                state.update_camera_buffer(*camera);

                update_required_camera = true;
            }
        }

        prev_frame_instant = frame_instant;
        prev_frame_duration += temp;

        #[allow(unused_mut)] // Indicates that its time for the next frame
        let mut update_required_framerate = false;

        #[cfg(not(target_arch = "wasm32"))] {
            // Calculate time since last resize event
            let resize_duration = resize_instant
                .signed_duration_since(frame_instant);

            let resize_duration = resize_duration
                .num_microseconds()
                .map(|micros| 0.001 * micros as f64)
                .unwrap_or(resize_duration.num_milliseconds() as f64)
                .abs();

            // If the user is done resizing, adjust texture and uniforms
            if resize_duration > frame_duration {
                if let Some(dim) = resize_dim.take() {
                    state.resize(*config, dim);

                    // We want to begin an update immediately after resizing
                    // update_required_framerate is co-opted for this purpose
                    update_required_framerate = true;
                }
            }
        }

        if !(update_required_camera || update_required_framerate) {
            // If no update is required, discard the frame
            if prev_frame_duration > frame_duration {
                prev_frame_duration -= frame_duration;
            }
        }

        // Force an update if `web` requests it
        if update_required_web && prev_frame_duration < frame_duration {
            prev_frame_duration += frame_duration;
        }

        let mut requested = false;
        while prev_frame_duration > frame_duration {
            // This is platform-specific
            // Queue::on_submitted_work_done is not available on WASM
            // So we implement compute pass completion checking with map_async
            state.update(*config);

            if !requested {
                // Anytime we update, we need to request a redraw
                window.request_redraw();

                // Don't submit any more requests
                requested = true;
            }
            
            // We need to state that we've handled the camera update
            // Since it resides outside of the event loop
            update_required_camera = false;

            // Decrement the frame
            prev_frame_duration -= frame_duration;
        }

        // If we've ran into an error, start the process of exiting
        if failure.is_err() { target.exit(); }
    }))?;

    failure
}