pub mod geom;
pub mod pipelines;
pub mod scene;
pub mod vertex;
pub mod state;
pub mod handlers;
pub mod shaders;

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
            simple_logger::SimpleLogger::new().init().unwrap();
        }
    }    
    
    let event_loop = BAIL(event_loop::EventLoop::new())?;
        event_loop.set_control_flow(event_loop::ControlFlow::Poll);

    let window = BAIL({
        window::WindowBuilder::new().build(&event_loop)
    })?;

    // This needs to be shared with State
    let window = sync::Arc::new(window);

    BAIL(event_loop.run(|event, target| {
        unsafe {
            if CONFIG.updated {
                CONFIG.updated = false;
                
                // TODO: State::update_config
            }
        }

        match event {
            event::Event::WindowEvent { event, window_id, .. }
                if window_id == window.id() => match event {
                    event::WindowEvent::CloseRequested | //
                    event::WindowEvent::KeyboardInput {
                        event: event::KeyEvent {
                            state: event::ElementState::Pressed,
                            logical_key: keyboard::Key::Named(keyboard::NamedKey::Escape), ..
                        }, ..
                    } => target.exit(),
                    event::WindowEvent::Resized(physical_size) => {
                        log::info!("{:?}", physical_size);
                    },
                    event::WindowEvent::RedrawRequested => {
                        // TODO
                    },
                    _ => { /*  */ },
                }
            ,
            _ => { /*  */ },
        }
    }))
}
