use std::{fmt, error};

use winit::{dpi, window};
use winit::platform::web::WindowExtWebSys as _;

use wasm_bindgen::prelude::*;

#[derive(Debug)]
struct WebError { op: &'static str, }

impl WebError {
    const fn new(op: &'static str) -> Self {
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

pub fn init(config: crate::Config, window: &window::Window) -> anyhow::Result<()> {    
    let dom = web_sys::window()
        .ok_or(WebError::new("obtain window"))?;

    let doc = dom.document()
        .ok_or(WebError::new("obtain document"))?;

    let elem: web_sys::Element = window
        .canvas()
        .ok_or(WebError::new("construct canvas element"))?
        .into();

    let elem_handle = config.canvas_raw_handle;
    let elem_handle = format!("{}", elem_handle);

    elem.set_attribute("data-raw-handle", &elem_handle)
        .map_err(|_| WebError::new("set data attribute"))?;

    doc.body()
        .ok_or(WebError::new("get <body> element"))?
        .append_child(&elem.into())
        .map_err(|_| WebError::new("append canvas to <body>"))?;

    Ok(())
}

// The Config global that the JS interface writes into
pub static mut CONFIG: crate::Config = crate::Config::new();

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

// Written to by `update_viewport` on WASM
pub static mut VIEWPORT: Option<dpi::PhysicalSize<u32>> = None;

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
