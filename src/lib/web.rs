use winit::{dpi, window};
use winit::platform::web::WindowExtWebSys as _;

use wasm_bindgen::prelude::*;

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


// The Config global that the JS interface writes into
pub static mut CONFIG: crate::Config = crate::Config::new();

// Written to by `update_viewport` on WASM
pub static mut VIEWPORT: Option<dpi::PhysicalSize<u32>> = None;

// Initialize all web-related stuff
pub fn init(
    config: crate::Config, 
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
pub unsafe fn update<H>(state: &mut crate::state::State<H>)
    where H: crate::handlers::IntrsHandler {

    if CONFIG.updated {
        CONFIG.updated = false;
        
        state.update_config(CONFIG.compute);
    }

    if let Some(size) = VIEWPORT.take() {
        state.resize(CONFIG, size);
    }
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn update_config(serialized: JsValue) -> Result<(), crate::Failed> {
    use std::io;

    match serialized.as_string() {
        Some(temp) => unsafe {
            CONFIG = crate::BAIL(serde_json::from_str::<crate::Config>(&temp))?;
        },
        None => {
            crate::BAIL(Err(io::Error::from(io::ErrorKind::InvalidData)))?;
        },
    }

    Ok(())
}

#[no_mangle]
#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn update_viewport(serialized: JsValue) -> Result<(), crate::Failed> {
    use std::io;

    match serialized.as_string() {
        Some(temp) => unsafe {
            let size = crate::BAIL({
                serde_json::from_str::<dpi::PhysicalSize<u32>>(&temp)
            })?;

            let _ = VIEWPORT.insert(size);
        },
        None => {
            crate::BAIL(Err(io::Error::from(io::ErrorKind::InvalidData)))?;
        },
    }

    Ok(())
}