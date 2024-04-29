mod package;

use std::{marker, sync};

use wgpu::util::DeviceExt as _;

use winit::{dpi, window};

use crate::{handlers, shaders, vertex};
use crate::scene;

#[derive(Debug)]
pub struct State<H: handlers::IntrsHandler> {
    // WGPU interface
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

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

    // Window
    window_size: winit::dpi::PhysicalSize<u32>,

    // PhantomData
    _p: marker::PhantomData<fn() -> H>,
}

impl<H: handlers::IntrsHandler> State<H> {
    const TEXTURE_FORMAT: wgpu::TextureFormat = //
        wgpu::TextureFormat::Rgba8Unorm;

    pub async fn new(
        config: crate::Config, 
        scene: &scene::Scene,
        window: sync::Arc<window::Window>,
    ) -> anyhow::Result<Self> {
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

        // Helper function to construct the surface target on WASM
        // It depends on the canvas having a particular data field
        #[cfg(target_arch = "wasm32")]
        unsafe fn target(config: crate::Config) -> anyhow::Result<wgpu::SurfaceTargetUnsafe> {
            use wgpu::rwh;

            Ok(wgpu::SurfaceTargetUnsafe::RawHandle { 
                raw_display_handle: rwh::RawDisplayHandle::Web({
                    rwh::WebDisplayHandle::new()
                }),
                raw_window_handle: rwh::RawWindowHandle::Web({
                    rwh::WebWindowHandle::new(config.canvas_raw_handle)
                }),
            })
        }

        cfg_if::cfg_if! {
            if #[cfg(target_arch="wasm32")] {
                let surface_target = unsafe { target(config)? };

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

        // TODO: In the future we want to enable TIMESTAMP_QUERY
        let device_desc = wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        };

        let (device, queue) = adapter
            .request_device(&device_desc, None)
            .await
            .unwrap();

        // Construct the size
        let size = match config.resolution {
            crate::Resolution::Dynamic(_) => window_size,
            crate::Resolution::Sized(size) => size,
            crate::Resolution::Fixed { size, .. } => size,
        };

        // Since we are using winit::dpi::PhysicalSize<u32>
        // instead of our own type, we have to manually cast it into a slice
        let size_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[size.width, size.height]),
                usage: wgpu::BufferUsages::UNIFORM 
                     | wgpu::BufferUsages::COPY_DST,
            }
        );

        // Get all the buffers, groups associated with the scene
        // These fill group(3)
        let scene::ScenePack {
            camera_buffer: scene_camera_buffer, 
            buffers: scene_buffers,
            bg: scene_group, 
            bg_layout: scene_group_layout, ..
        } = scene.pack(&device);

        // We have to hold onto the Config buffer since it can be updated live
        let config_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[config.compute]),
                usage: wgpu::BufferUsages::UNIFORM 
                     | wgpu::BufferUsages::COPY_DST,
            }
        );

        let config_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
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
                ],
            }
        );

        let config_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &config_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: config_buffer.as_entire_binding(),
                    },
                ],
            }
        );

        let surface_capabilities = surface.get_capabilities(&adapter);

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let format = Self::TEXTURE_FORMAT;
            } else {
                let format = Self::TEXTURE_FORMAT.add_srgb_suffix();
            }
        }

        if !surface_capabilities.formats.contains(&format) {
            anyhow::bail!(wgpu::SurfaceError::Lost);
        }

        let wgpu::SurfaceCapabilities {
            present_modes,
            alpha_modes, ..
        } = surface_capabilities;

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

        surface.configure(&device, &surface_config);

        // Build the render shader module
        let shader_render = device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: None,
                source: shaders::source::<H>(shaders::ShaderStage::Render)?,
            },
        );

        // Collection of IntrsHandler-specific bindings
        let pack = H::vars(scene, &device)?;

        // The compute shader module requires workgroup size 
        // and the variable pack
        let shader_compute = device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: None,
                source: shaders::source::<H>(shaders::ShaderStage::Compute {
                    wg: config.resolution.wg(),
                    pack: &pack,
                })?,
            },
        );

        // The vertices for the screen-space quad
        let vertices = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertex::CLIP_SPACE_EXTREMA),
                usage: wgpu::BufferUsages::VERTEX
            }
        );

        // Corresponding indices for screen-space quad
        let indices = device.create_buffer_init(
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
            &device, 
            Self::TEXTURE_FORMAT,
            &shader_compute, 
            &shader_render, 
            size,
            &size_buffer,
            layouts.as_slice(),
        );

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,

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

            window_size,

            _p: marker::PhantomData,
        })
    }

    pub fn resize_hard(&mut self, size: dpi::PhysicalSize<u32>) {
        self.queue.write_buffer(
            &self.size_buffer, 
            0,
            bytemuck::cast_slice(&[size.width, size.height])
        );

        let Self {
            device,
            shader_compute,
            shader_render,
            pack: handlers::IntrsPack { vars, layout, .. }, 
            size_buffer,
            scene_group_layout,
            config_group_layout,  ..
        } = self;

        
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
            Self::TEXTURE_FORMAT,
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
        if size.width > 0 && size.height > 0 {
            self.window_size = size;

            self.surface_config.width = size.width;
            self.surface_config.height = size.height;

            self.surface.configure(&self.device, &self.surface_config);

            if let crate::Resolution::Dynamic { .. } = config.resolution {
                self.resize_hard(size);
            }
        }
    }

    pub fn update(&mut self, config: crate::Config) {
        let mut encoder = self.device.create_command_encoder(&{
            wgpu::CommandEncoderDescriptor::default()
        });

        {
            let compute_pass_descriptor = //
                wgpu::ComputePassDescriptor::default();

            let mut compute_pass = encoder
                .begin_compute_pass(&compute_pass_descriptor);

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
                crate::Resolution::Dynamic { .. } => self.window_size,
                crate::Resolution::Sized(size) => size,
                crate::Resolution::Fixed { size, .. } => size,
            };

            compute_pass.dispatch_workgroups(
                width.div_euclid(wg), 
                height.div_euclid(wg), 
                1
            );
        }

        self.queue.submit(Some(encoder.finish()));
    }

    pub fn update_camera_buffer(&mut self, camera: scene::CameraUniform) {
        self.queue.write_buffer(
            &self.scene_camera_buffer, 0, 
            bytemuck::cast_slice(&[camera])
        );
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update_config(&mut self, config: crate::ComputeConfig) {
        self.queue.write_buffer(
            &self.config_buffer, 0,
            bytemuck::cast_slice(&[config])
        );
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update_scene(&mut self, scene: &scene::Scene) {
        use std::mem;
        
        let scene::ScenePack {
            camera_buffer,
            buffers,
            bg, ..
        } = scene.pack(&self.device);

        let _ = mem::replace(&mut self.scene_group, bg);
        let _ = mem::replace(&mut self.scene_camera_buffer, camera_buffer);

        for (dst, src) in self.scene_buffers.iter_mut().zip(buffers) {
            mem::replace(dst, src).destroy();
        }
    }

    pub fn window_size(&self) -> dpi::PhysicalSize<u32> {
        self.window_size
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder({
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
        self.queue.submit(Some(encoder.finish()));

        // Schedule for drawing
        output.present();

        Ok(())
    }
}