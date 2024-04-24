use winit::dpi;

use crate::pipelines;

pub struct PipelinePackage {
    pub compute_group: wgpu::BindGroup,
    pub compute_pipeline: wgpu::ComputePipeline,
    pub render_group: wgpu::BindGroup,
    pub render_pipeline: wgpu::RenderPipeline,
}

impl PipelinePackage {
    pub fn new(
        device: &wgpu::Device,
        tex_format: wgpu::TextureFormat,
        shader_compute: &wgpu::ShaderModule,
        shader_render: &wgpu::ShaderModule,
        size: dpi::PhysicalSize<u32>,
        size_buffer: &wgpu::Buffer,
        layouts: &[&wgpu::BindGroupLayout],
    ) -> Self {
        let dpi::PhysicalSize {
            width,
            height, ..
        } = size;

        // Init the texture
        let texture = device.create_texture(
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: tex_format,
                usage: wgpu::TextureUsages::STORAGE_BINDING 
                     | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[
                    tex_format,
                    #[cfg(not(target_arch = "wasm32"))] // TODO: See if this can be removed
                    tex_format.add_srgb_suffix(),
                ],
            }
        );

        // The SRGB texture view isn't available on web
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let tex_view_render_format = tex_format;
            } else {
                let tex_view_render_format = tex_format.add_srgb_suffix();
            }
        }

        let tex_view_render = texture.create_view(
            &wgpu::TextureViewDescriptor {
                label: None,
                format: Some(tex_view_render_format),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
            }
        );

        let tex_view_compute = texture.create_view(
            &wgpu::TextureViewDescriptor {
                label: None,
                format: Some(tex_format),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
            }
        );

        // Build the compute pipeline
        let builder = pipelines::PipelineBuilder {
            device,
            tex_format,
            tex_view: &tex_view_compute,
            module: shader_compute,
            size: size_buffer,
            layouts,
        };

        let pipelines::Pipeline {
            inner: compute_pipeline,
            group: compute_group, ..
        } = builder.into();

        // Build the render pipeline
        let builder = pipelines::PipelineBuilder {
            device,
            tex_format,
            tex_view: &tex_view_render,
            module: shader_render,
            size: size_buffer,
            layouts: &[],
        };

        let pipelines::Pipeline {
            inner: render_pipeline,
            group: render_group, ..
        } = builder.into();

        // Assemble and return
        Self {
            compute_group,
            compute_pipeline,
            render_group,
            render_pipeline,
        }
    }
}