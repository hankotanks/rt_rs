use crate::vertex;

pub struct Pipeline<P> {
    pub inner: P,
    pub group: wgpu::BindGroup,
}

#[derive(Clone, Copy)]
pub struct PipelineBuilder<'a> {
    pub device: &'a wgpu::Device,
    pub tex_format: wgpu::TextureFormat,
    pub tex_view: &'a wgpu::TextureView,
    pub size: &'a wgpu::Buffer,
    pub module: &'a wgpu::ShaderModule,
    pub layouts: &'a [&'a wgpu::BindGroupLayout],
}

#[allow(clippy::from_over_into)]
impl<'a> Into<Pipeline<wgpu::ComputePipeline>> for PipelineBuilder<'a> {
    fn into(self) -> Pipeline<wgpu::ComputePipeline> {
        let Self {
            device,
            tex_format, 
            tex_view,
            size,
            module,
            layouts, ..
        } = self;

        let compute_tex_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: tex_format,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            }
        );
    
        let group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &compute_tex_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(tex_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: size.as_entire_binding(),
                    },
                ],
            }
        );
    
        let inner_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: None,
                push_constant_ranges: &[],
                bind_group_layouts: [
                    &[&compute_tex_group_layout],
                    layouts
                ].concat().as_slice(),
            } 
        );
    
        let inner = device.create_compute_pipeline(
            &wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&inner_layout),
                module,
                entry_point: "main_cs",
            }
        );

        Pipeline { inner, group, }
    }
}

#[allow(clippy::from_over_into)]
impl<'a> Into<Pipeline<wgpu::RenderPipeline>> for PipelineBuilder<'a> {
    fn into(self) -> Pipeline<wgpu::RenderPipeline> {
        let Self {
            device,
            tex_format,
            tex_view,
            size,
            module, ..
        } = self;
        
        let tg_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { 
                                filterable: false 
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        count: None,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        }
                    }
                ],
            }
        );
    
        let tg = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &tg_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(tex_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: size.as_entire_binding(),
                    },
                ],
            }
        );
    
        let inner_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: None,
                push_constant_ranges: &[],
                bind_group_layouts: &[&tg_layout],
            }
        );

        // The SRGB texture view isn't available on web
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let inner_fragment_format = tex_format;
            } else {
                let inner_fragment_format = tex_format.add_srgb_suffix();
            }
        }
    
        let inner = device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&inner_layout),
                vertex: wgpu::VertexState {
                    module: self.module,
                    entry_point: "vs_main",
                    buffers: &[vertex::Vertex::description()],
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: inner_fragment_format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::REPLACE,
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            }
        );
    
        Pipeline { inner, group: tg, }
    }
}