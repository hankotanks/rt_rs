use std::io;

use winit::{dpi, window};
use winit::platform::web::WindowExtWebSys as _;

use crate::{state, scene, handlers, timing};

mod err {
    use std::{fmt, error};

    #[derive(Debug)]
    pub struct WebError { op: &'static str, }
    
    impl WebError {
        pub const fn new(op: &'static str) -> Self {
            Self { op }
        }
    }
    
    impl fmt::Display for WebError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Unable to {}.", self.op)
        }
    }
    
    impl error::Error for WebError {
        fn source(&self) -> Option<&(dyn error::Error + 'static)> { None }
        fn cause(&self) -> Option<&dyn error::Error> { self.source() }
    }
}

pub type WebHandler = handlers::BasicIntrs;

pub struct WebState {
    // These members are used for run_internal dispatch
    pub config: crate::Config<WebHandler>,

    // Scene no longer carries the IntrsHandler
    pub scene: scene::Scene,

    // Information related to updates
    update_config: bool,
    update_scene: bool,

    // This value is only set when a resize event has occurred
    viewport: Option<dpi::PhysicalSize<u32>>,
}

pub static mut WEB_STATE: WebState = WebState {
    config: crate::Config::<WebHandler>::new(),
    update_config: true,
    scene: scene::Scene::Unloaded,
    update_scene: false,
    viewport: None,
};

// Initialize all web-related stuff
pub fn init<H: handlers::IntrsHandler>(
    config: crate::Config<H>, 
    window: &window::Window
) -> anyhow::Result<()> {    
    let dom = web_sys::window()
        .ok_or(err::WebError::new("obtain window"))?;

    let doc = dom.document()
        .ok_or(err::WebError::new("obtain document"))?;

    let elem: web_sys::Element = window
        .canvas()
        .ok_or(err::WebError::new("construct canvas element"))?
        .into();

    let elem_handle = config.canvas_raw_handle;
    let elem_handle = format!("{}", elem_handle);

    elem.set_attribute("data-raw-handle", &elem_handle)
        .map_err(|_| err::WebError::new("set data attribute"))?;

    doc.body()
        .ok_or(err::WebError::new("get <body> element"))?
        .append_child(&elem.into())
        .map_err(|_| err::WebError::new("append canvas to <body>"))?;

    Ok(())
}

// Update all web-related stuff
// Returns true if a re-render is necessary
pub unsafe fn update<S>(state: &mut state::State<S>) -> bool 
    where S: timing::Scheduler {

    let mut update = false;

    if WEB_STATE.update_config {
        WEB_STATE.update_config = false;
        
        state.update_config(WEB_STATE.config.compute);

        update = true;
    }

    if WEB_STATE.update_scene {
        WEB_STATE.update_scene = false;

        state.update_scene(&(WEB_STATE.scene));

        update = true;
    }

    if let Some(size) = WEB_STATE.viewport.take() {
        state.resize(WEB_STATE.config, size);

        update = true;
    }

    update
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn update_config(
    serialized: wasm_bindgen::JsValue
) -> Result<(), crate::Failed> {
    match serialized.as_string() {
        Some(temp) => unsafe {
            WEB_STATE.config = crate::BAIL({
                serde_json::from_str::<crate::Config<WebHandler>>(&temp)
            })?;

            WEB_STATE.update_config = true;
        },
        None => {
            crate::BAIL(Err(io::Error::from(io::ErrorKind::InvalidData)))?;
        },
    }

    Ok(())
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn update_scene(
    serialized: wasm_bindgen::JsValue
) -> Result<(), crate::Failed> {
    match serialized.as_string() {
        Some(temp) => unsafe {
            WEB_STATE.scene = crate::BAIL({
                serde_json::from_str::<scene::Scene>(&temp)
            })?;

            WEB_STATE.update_scene = true;
        },
        None => {
            crate::BAIL(Err(io::Error::from(io::ErrorKind::InvalidData)))?;
        },
    }

    Ok(())
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn update_viewport(
    serialized: wasm_bindgen::JsValue
) -> Result<(), crate::Failed> {
    match serialized.as_string() {
        Some(temp) => unsafe {
            let size = crate::BAIL({
                serde_json::from_str::<dpi::PhysicalSize<u32>>(&temp)
            })?;

            let _ = WEB_STATE.viewport.insert(size);
        },
        None => {
            crate::BAIL(Err(io::Error::from(io::ErrorKind::InvalidData)))?;
        },
    }

    Ok(())
}