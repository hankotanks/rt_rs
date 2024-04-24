#[derive(Debug)]
struct WebError { op: &'static str, }

impl WebError {
    const fn new(op: &'static str) -> Self {
        Self { op }
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to {}", self.op)
    }
}

impl std::error::Error for WebError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

pub fn init(config: crate::Config, window: &winit::window::Window) -> anyhow::Result<()> {
    use winit::dpi::PhysicalSize;
    use winit::window::Window;
    use winit::platform::web::WindowExtWebSys;

    use wasm_bindgen::JsCast;
    
    let dom = web_sys::window()
        .ok_or(WebError::new("obtain window"))?;

    let doc = dom.document()
        .ok_or(WebError::new("obtain document"))?;

    let elem = window
        .canvas()
        .ok_or(WebError::new("construct canvas"))?;

    let elem = web_sys::Element::from(elem);

    let elem_handle = config.canvas_raw_handle;
    let elem_handle = format!("{}", elem_handle);

    elem.set_attribute("data-raw-handle", &elem_handle)
        .map_err(|_| WebError::new("set data attribute"))?;

    doc.get_elements_by_name("body")
        .item(0)
        .ok_or(WebError::new("get body element"))?
        .append_child(&elem.into())
        .map_err(|_| WebError::new("append canvas to <body>"))?;

    Ok(())
}

