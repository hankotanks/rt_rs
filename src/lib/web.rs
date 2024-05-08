use winit::{dpi, window};

use crate::{state, scene, handlers, timing};

mod err {
    use std::{fmt, error};

    #[derive(Debug)]
    pub struct WebError { 
        op: &'static str, 
    }
    
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
        fn source(&self) -> Option<&(dyn error::Error + 'static)> { 
            None 
        }

        fn cause(&self) -> Option<&dyn error::Error> { 
            self.source() 
        }
    }
}

pub type WebHandler = handlers::BvhIntrs;

pub struct WebState {
    // These members are used for run_internal dispatch
    pub config: crate::Config,

    // Scene no longer carries the IntrsHandler
    pub scene: scene::Scene,
    pub scene_temp: Option<scene::Scene>,

    // These flags tell us when there is an update pending
    update_config: bool,

    // This value is only set when a resize event has occurred
    viewport: Option<dpi::PhysicalSize<u32>>,
}

pub static mut WEB_STATE: WebState = WebState {
    config: crate::Config::new(),
    update_config: true,
    scene: scene::Scene::Unloaded,
    scene_temp: None,
    viewport: None,
};

// Initialize all web-related stuff
pub fn init(window: &window::Window) -> anyhow::Result<()> {
    use winit::platform::web::WindowExtWebSys as _;

    // Obtain the window
    let dom = web_sys::window()
        .ok_or(err::WebError::new("obtain window"))?;

    // Get the document
    let doc = dom.document()
        .ok_or(err::WebError::new("obtain document"))?;

    // Build the canvas from the winit::window::Window
    let elem: web_sys::Element = window
        .canvas()
        .ok_or(err::WebError::new("construct canvas element"))?
        .into();

    // Add the handle so we can find it from within state
    elem.set_attribute("data-raw-handle", "2024")
        .map_err(|_| err::WebError::new("set data attribute"))?;

    // Insert the canvas into the body
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

    if let Some(scene) = WEB_STATE.scene_temp.take() {
        update = match state.load::<WebHandler>(
            WEB_STATE.config, 
            <WebHandler as handlers::IntrsHandler>::Config::default(),
            &scene
        ) {
            Ok(_) => {
                WEB_STATE.scene = scene; true
            },
            Err(_) => false,
        };
    }

    if let Some(size) = WEB_STATE.viewport.take() {
        state.resize(WEB_STATE.config, size);

        update = true;
    }

    update
}

unsafe fn parse<'de, 'a: 'de, D>(
    serialized: wasm_bindgen::JsValue
) -> Result<D, wasm_bindgen::JsValue>
    where D: serde::Deserialize<'de> {
    
    use std::mem;

    match serialized.as_string() {
        Some(temp) => {
            let temp = mem::transmute::<&str, &'a str>(&temp);

            serde_json::from_str::<D>(temp)
                .map_err(|_| serialized)
        },
        None => Err(serialized)
    }
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub unsafe fn update_config(
    serialized: wasm_bindgen::JsValue
) -> Result<(), wasm_bindgen::JsValue> {
    WEB_STATE.config = parse::<crate::Config>(serialized)?;

    WEB_STATE.update_config = true;

    Ok(())
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub unsafe fn update_scene(
    serialized: wasm_bindgen::JsValue
) -> Result<(), crate::Failed> {
    let _ = WEB_STATE.scene_temp.insert(parse::<scene::Scene>(serialized)?);

    Ok(())
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub unsafe fn update_viewport(
    serialized: wasm_bindgen::JsValue
) -> Result<(), crate::Failed> {
    let _ = WEB_STATE.viewport.insert({
        parse::<dpi::PhysicalSize<u32>>(serialized)?
    });

    Ok(())
}