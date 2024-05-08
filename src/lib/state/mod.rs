mod package;

use std::{mem, sync};

use winit::{dpi, window};

use crate::{handlers, scene, shaders, timing, vertex};

#[derive(Debug)]
struct StateInternals {
    window_size: dpi::PhysicalSize<u32>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

impl StateInternals {
    const TEXTURE_FORMAT: wgpu::TextureFormat = //
        wgpu::TextureFormat::Rgba8Unorm;

    async fn new(window: sync::Arc<window::Window>) -> anyhow::Result<Self> {
        let window_size = match window.inner_size() {
            // This value can later be used as an Extent3D for a texture
            // We never want texture dimensions to be 0,
            // so we set it to (1, 1). On all targets this value is updated after
            // the first frame
            dpi::PhysicalSize { width: 0, .. } | //
            dpi::PhysicalSize { height: 0, .. } => dpi::PhysicalSize::new(1, 1),
            window_size => window_size
        };

        // NOTE: Specifying `wgpu::Backends::BROWSER_WEBGPU`
        // ensures that WGPU never chooses WebGL2
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let backends = wgpu::Backends::BROWSER_WEBGPU;
            } else {
                let backends = wgpu::Backends::all();
            }
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends, ..Default::default()
        });

        cfg_if::cfg_if! {
            if #[cfg(target_arch="wasm32")] {
                use wgpu::rwh;
                let surface_target = wgpu::SurfaceTargetUnsafe::RawHandle { 
                    raw_display_handle: rwh::RawDisplayHandle::Web({
                        rwh::WebDisplayHandle::new()
                    }),
                    raw_window_handle: rwh::RawWindowHandle::Web({
                        // NOTE: This id is hard-coded
                        rwh::WebWindowHandle::new(2024)
                    }),
                };

                let surface = unsafe {
                    instance.create_surface_unsafe(surface_target)?
                };
            } else {
                let surface_target = Box::new(window.clone());
                let surface_target = wgpu::SurfaceTarget::Window(surface_target);

                let surface = instance.create_surface(surface_target)?;
            }
        }

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let required_features = wgpu::Features::empty();
            } else {
                let required_features = wgpu::Features::TIMESTAMP_QUERY;
            }
        }

        // TODO: In the future we want to enable TIMESTAMP_QUERY
        let device_desc = wgpu::DeviceDescriptor {
            label: None,
            required_features,
            required_limits: wgpu::Limits::default(),
        };

        let (device, queue) = adapter
            .request_device(&device_desc, None)
            .await
            .unwrap();

            let surface_capabilities = surface.get_capabilities(&adapter);

            cfg_if::cfg_if! {
                if #[cfg(target_arch = "wasm32")] {
                    let format = Self::TEXTURE_FORMAT;
                } else {
                    let format = Self::TEXTURE_FORMAT.add_srgb_suffix();
                }
            }
    
            // Bail immediately if we don't support the given format
            if !surface_capabilities.formats.contains(&format) {
                anyhow::bail!(wgpu::SurfaceError::Lost);
            }
    
            let wgpu::SurfaceCapabilities {
                present_modes,
                alpha_modes, ..
            } = surface_capabilities;
    
            // Construct the surface configuration
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: window_size.width,
                height: window_size.height,
                present_mode: present_modes[0],
                alpha_mode: alpha_modes[0],
                view_formats: vec![
                    Self::TEXTURE_FORMAT,
                    Self::TEXTURE_FORMAT.add_srgb_suffix(),
                ],
                desired_maximum_frame_latency: 1,
            };
    
            // Configure the surface (no longer platform-specific)
            surface.configure(&device, &surface_config);
    
            Ok(Self {
                window_size,
                device,
                queue,
                surface,
                surface_config,
            })
    }
}

#[derive(Debug)]
pub struct State<S: timing::Scheduler> {
    // WGPU interface
    internals: Option<StateInternals>,

    // Keeps track of frame time
    scheduler: S,

    // CPU-side of the intersection logic
    pack: handlers::IntrsPack<'static>,

    // Shader modules
    shader_compute: wgpu::ShaderModule,
    shader_render: wgpu::ShaderModule,

    // Size buffer
    // NOTE: Included in `compute_group`
    size_buffer: wgpu::Buffer,

    // Scene buffers & group
    scene_group_layout: wgpu::BindGroupLayout,
    scene_group: wgpu::BindGroup,
    scene_camera_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    scene_buffers: Vec<wgpu::Buffer>,

    // Config buffers & group
    #[allow(dead_code)]
    config_buffer: wgpu::Buffer,
    config_group_layout: wgpu::BindGroupLayout,
    config_group: wgpu::BindGroup,

    // Texture binding group and compute pipeline
    compute_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    
    // Render pass
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    render_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

impl<S: timing::Scheduler> State<S> {
    pub async fn new<H: handlers::IntrsHandler>(
        config: crate::Config, 
        config_handler: H::Config,
        scene: &scene::Scene,
        window: sync::Arc<window::Window>,
    ) -> anyhow::Result<Self> {
        // We only build this once
        // All other state loads pass it back and forth
        let internals = StateInternals::new(window).await?;

        // Helper function to help with branching caused by errors
        fn new_internal<S: timing::Scheduler, H: handlers::IntrsHandler>(
            internals: StateInternals,
            config: crate::Config, 
            scene: &scene::Scene,
            handler: H,
        ) -> anyhow::Result<State<S>> {
            // If state construction fails at first, we go through the following:
            // 1. Try it again with a blank handler
            // 2. Try it again with a blank scene
            match State::init(internals, config, scene, handler) {
                Ok(state) => Ok(state),
                Err((internals, e0)) => {
                    // Bail if the scene was unloaded.
                    // There's nothing else to do to save it
                    if matches!(scene, scene::Scene::Unloaded) {
                        anyhow::bail!(e0);
                    }

                    // If the scene was active and failed,
                    // we can try again with an unloaded one
                    match State::init(
                        internals, 
                        config, 
                        &scene::Scene::Unloaded, 
                        H::new(H::Config::default()).unwrap()
                    ) {
                        Ok(state) => {
                            // Inform the user that the event loop hasn't exited
                            // as a result of the error
                            #[cfg(target_arch = "wasm32")]
                            crate::web::note("\
                                Scene construction failed. \
                                Initialization will continue with a blank scene\
                            ")?;

                            Ok(state)
                        },
                        Err((_, e1)) => {
                            // Throw error, 
                            // failure is too catastrophic to continue
                            anyhow::bail!(e1.context(e0));
                        },
                    }
                },
            }
        }

        // Build the handler
        match H::new(config_handler) {
            Ok(handler) => new_internal(internals, config, scene, handler),
            Err(_) => {
                use handlers::{BlankIntrs, IntrsHandler};

                // If handler construction fails, 
                // try it again with a blank handler
                #[cfg(target_arch = "wasm32")]
                crate::web::note("\
                    Failed to initialize intersection handler. \
                    Attempting to substitute a blank handler\
                ")?;

                // Build the handler
                let handler = <BlankIntrs as IntrsHandler>::new(())
                    .unwrap();

                // Try to construct state
                new_internal(internals, config, scene, handler)
            },
        }
    }

    // This function replaces self with a new state object
    // (that has initialized a new scene's data)
    #[cfg(target_arch = "wasm32")]
    pub fn load<H: handlers::IntrsHandler>(
        &mut self, 
        config: crate::Config, 
        config_handler: H::Config,
        scene: &scene::Scene,
    ) -> anyhow::Result<()> {
        let internals = self.internals
            .take()
            .unwrap();

        fn destroy<S: timing::Scheduler>(state: &State<S>) {
            let State {
                pack,
                scene_camera_buffer,
                scene_buffers,
                config_buffer, ..
            } = state;
    
            // The CPU-side intersection buffers
            pack.destroy();
    
            // The Camera uniform buffer
            scene_camera_buffer.destroy();
            
            // Each buffer in the scene
            // Includes prims, vertices, etc.
            for buffer in scene_buffers {
                buffer.destroy();
            }
    
            // The ComputeConfig buffer
            config_buffer.destroy();
        }

        match H::new(config_handler) {
            Ok(handler) => {
                match Self::init::<H>(internals, config, scene, handler) {
                    Ok(state) => {
                        destroy(self); 
                        
                        let _ = mem::replace(self, state);
                    },
                    Err((internals, e)) => {
                        #[cfg(target_arch = "wasm32")]
                        crate::web::note("Failed to initialize the selected scene")?;

                        let _ = self.internals.insert(internals); Err(e)?;
                    },
                }
            },
            Err(e) => {
                // If handler construction fails, 
                // try it again with a blank handler
                #[cfg(target_arch = "wasm32")]
                crate::web::note("Failed to initialize intersection handler")?;

                let _ = self.internals.insert(internals); Err(e)?;
            },
        }

        Ok(())
    }

    // NOTE: This return type is a little strange,
    // because we need to recover the StateInternals if
    // initialization fails.
    // By doing this, we can keep the program going on the current scene
    fn init<H: handlers::IntrsHandler>(
        internals: StateInternals,
        config: crate::Config,
        scene: &scene::Scene,
        handler: H,
    ) -> Result<Self, (StateInternals, anyhow::Error)> {
        use wgpu::util::DeviceExt as _;

        // Frame scheduler + benchmark handler
        let scheduler = S::init(&internals.queue, &internals.device);

        // Construct the size
        let size = match config.resolution {
            crate::Resolution::Dynamic(_) => internals.window_size,
            crate::Resolution::Sized(size) => size,
            crate::Resolution::Fixed { size, .. } => size,
        };

        // Since we are using winit::dpi::PhysicalSize<u32>
        // instead of our own type, we have to manually cast it into a slice
        let size_buffer = internals.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[size.width, size.height]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        // Get all the buffers, groups associated with the scene
        // These fill group(3)
        let scene::ScenePack {
            camera_buffer: scene_camera_buffer, 
            buffers: scene_buffers,
            bg: scene_group, 
            bg_layout: scene_group_layout, ..
        } = scene.pack(&internals.device);

        // We have to hold onto the Config buffer since it can be updated live
        let config_buffer = internals.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[config.compute]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        // A list of all entry layouts in the config group (2)
        let mut config_group_layout_entries = vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                count: None,
                ty: wgpu::BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: wgpu::BufferBindingType::Uniform,
                }
            },
        ];

        // A list of all entries in the config group (2)
        let mut config_group_entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: config_buffer.as_entire_binding(),
            },
        ];

        // The scheduler's buffers (if its using them)
        // need to piggyback off group 2
        // Otherwise, they will always be available to map
        if let Some(scheduler_entry) = scheduler.entry() {
            let timing::SchedulerEntry {
                ty,
                resource,
            } = scheduler_entry;

            config_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: config_group_layout_entries.len() as u32,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty,
                count: None,
            });

            config_group_entries.push(wgpu::BindGroupEntry {
                binding: config_group_entries.len() as u32,
                resource,
            });
        }

        let config_group_layout = internals.device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &config_group_layout_entries,
            }
        );

        let config_group = internals.device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &config_group_layout,
                entries: &config_group_entries,
            }
        );

        // Build the render shader module
        let shader_render = internals.device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: None,
                source: match shaders::source(shaders::ShaderStage::Render) {
                    Ok(source) => source,
                    Err(e) => { 
                        return Err((internals, e)); 
                    },
                },
            },
        );

        // Collection of IntrsHandler-specific bindings
        let pack = handler.vars(scene, &internals.device);

        // The compute shader module requires workgroup size 
        // and the variable pack
        let shader_compute = internals.device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: None,
                source: match shaders::source(shaders::ShaderStage::Compute {
                    wg: config.resolution.wg(),
                    pack: &pack,
                    logic: handler.logic(),
                }) {
                    Ok(source) => source,
                    Err(e) => {
                        return Err((internals, e));
                    }
                },
            },
        );

        // The vertices for the screen-space quad
        let vertices = internals.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertex::CLIP_SPACE_EXTREMA),
                usage: wgpu::BufferUsages::VERTEX
            }
        );

        // Corresponding indices for screen-space quad
        let indices = internals.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertex::INDICES),
                usage: wgpu::BufferUsages::INDEX
            }
        );

        let handlers::IntrsPack { 
            vars, 
            layout, .. 
        } = &pack;

        let layouts = if vars.is_empty() {
            vec![&config_group_layout, &scene_group_layout]
        } else {
            vec![&config_group_layout, &scene_group_layout, layout]
        };

        let package::PipelinePackage {
            compute_group,
            compute_pipeline,
            render_group,
            render_pipeline,
        } = package::PipelinePackage::new(
            &internals.device, 
            StateInternals::TEXTURE_FORMAT,
            &shader_compute, 
            &shader_render, 
            size,
            &size_buffer,
            layouts.as_slice(),
        );

        Ok(Self {
            internals: Some(internals),

            scheduler,

            pack,

            shader_compute,
            shader_render,

            size_buffer,

            scene_group_layout,
            scene_group,
            scene_camera_buffer,
            scene_buffers,

            config_buffer,
            config_group_layout,
            config_group,

            compute_group,
            compute_pipeline,

            vertices,
            indices,
            render_group,
            render_pipeline,
        })
    }

    pub fn resize_hard(&mut self, size: dpi::PhysicalSize<u32>) {
        let Self {
            internals: Some(StateInternals { device, queue, .. }),
            shader_compute,
            shader_render,
            pack: handlers::IntrsPack { vars, layout, .. }, 
            size_buffer,
            scene_group_layout,
            config_group_layout,  ..
        } = self else { unreachable!(); };

        queue.write_buffer(
            size_buffer, 
            0,
            bytemuck::cast_slice(&[size.width, size.height])
        );
        
        let layouts: Vec<&wgpu::BindGroupLayout> = if vars.is_empty() {
            vec![config_group_layout, scene_group_layout]
        } else {
            vec![config_group_layout, scene_group_layout, layout]
        };

        let package::PipelinePackage {
            compute_group,
            compute_pipeline,
            render_group,
            render_pipeline,
        } = package::PipelinePackage::new(
            device, 
            StateInternals::TEXTURE_FORMAT,
            shader_compute, 
            shader_render, 
            size, 
            size_buffer,
            layouts.as_slice(),
        );

        self.compute_group = compute_group;
        self.compute_pipeline = compute_pipeline;

        self.render_group = render_group;
        self.render_pipeline = render_pipeline;
    }

    pub fn resize(
        &mut self,
        config: crate::Config,
        size: winit::dpi::PhysicalSize<u32>
    ) {
        let Self {
            internals: Some(StateInternals { 
                window_size,
                device,
                surface,
                surface_config, ..
            }), ..
        } = self else { unreachable!(); };

        if size.width > 0 && size.height > 0 {
            let _ = mem::replace(window_size, size);

            surface_config.width = size.width;
            surface_config.height = size.height;

            surface.configure(device, surface_config);

            if let crate::Resolution::Dynamic { .. } = config.resolution {
                self.resize_hard(size);
            }
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let Self {
            internals: Some(StateInternals { 
                device, 
                queue, 
                surface, .. 
            }), ..
        } = self else { unreachable!(); };

        let output = surface.get_current_texture()?;

        let view = output.texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder({
            &wgpu::CommandEncoderDescriptor::default()
        });

        {
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            };

            // No need for fancy features like the depth buffer
            let mut render_pass = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    ..Default::default()
                }
            );

            // Apply the render pipeline
            render_pass.set_pipeline(&self.render_pipeline);

            // This contains the texture and size
            render_pass.set_bind_group(0, &self.render_group, &[]);

            // The indices for the screen-space quad
            render_pass.set_index_buffer(
                self.indices.slice(..), 
                wgpu::IndexFormat::Uint32
            );

            // The vertices for the screen-space quad
            render_pass.set_vertex_buffer(0, self.vertices.slice(..));

            render_pass.draw_indexed(
                0..(vertex::INDICES.len() as u32), 
                0, 
                0..1
            ); 
        }

        // Submit for execution (async)
        queue.submit(Some(encoder.finish()));

        // Schedule for drawing
        output.present();

        Ok(())
    }

    pub fn update(&mut self, config: crate::Config) {
        if self.scheduler.ready() {
            self.update_internal(config);
        }
    }

    fn update_internal(&mut self, config: crate::Config) {
        let Self {
            internals: Some(StateInternals { 
                device, 
                queue,
                window_size, .. 
            }), ..
        } = self else { unreachable!(); };

        let mut encoder = device.create_command_encoder(&{
            wgpu::CommandEncoderDescriptor::default()
        });

        {
            let mut compute_pass = encoder
                .begin_compute_pass(&self.scheduler.desc());

            compute_pass.set_pipeline(&self.compute_pipeline);

            let Self {
                config_group, 
                scene_group,
                compute_group,
                pack: handlers::IntrsPack { vars, group, .. }, ..
            } = self;

            compute_pass.set_bind_group(0, compute_group, &[]);
            compute_pass.set_bind_group(1, config_group, &[]);
            compute_pass.set_bind_group(2, scene_group, &[]);

            if !vars.is_empty() {
                compute_pass.set_bind_group(3, group, &[]);
            }

            let wg = config.resolution.wg();

            let dpi::PhysicalSize {
                width,
                height, ..
            } = match config.resolution {
                crate::Resolution::Dynamic { .. } => *window_size,
                crate::Resolution::Sized(size) => size,
                crate::Resolution::Fixed { size, .. } => size,
            };

            compute_pass.dispatch_workgroups(
                width.div_euclid(wg), 
                height.div_euclid(wg), 
                1
            );
        }

        self.scheduler.pre(&mut encoder);

        queue.submit(Some(encoder.finish()));

        self.scheduler.post(queue, device);
    }

    pub fn update_camera_buffer(&mut self, camera: scene::CameraUniform) {
        let Self {
            internals: Some(StateInternals { queue, .. }), 
            scene_camera_buffer, ..
        } = self else { unreachable!(); };

        queue.write_buffer(
            scene_camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera]),
        );
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update_config(&mut self, config: crate::ComputeConfig) {
        let Self {
            internals: Some(StateInternals { queue, .. }), 
            config_buffer, ..
        } = self else { unreachable!(); };

        queue.write_buffer(
            config_buffer, 0,
            bytemuck::cast_slice(&[config])
        );
    }
}