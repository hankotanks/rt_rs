// TODO: This should not be public
mod aabb;
pub use aabb::Aabb;
use wgpu::util::DeviceExt;

mod data;

use crate::geom;

#[repr(C)]
#[derive(Clone, Copy)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Debug)]
pub struct Bounds {
    min: [f32; 3],
    _p0: u32,
    max: [f32; 3],
    _p1: u32,
}

impl Bounds {
    fn new<P>(prims: P, vertices: &[geom::PrimVertex]) -> Self
        where P: Iterator<Item = geom::Prim> {

        let mut min = [std::f32::MAX; 3];
        let mut max = [std::f32::MAX * -1.; 3];

        fn extrema_vertex(
            vertex: [f32; 3], 
            minima: &mut [f32; 3], 
            maxima: &mut [f32; 3],
        ) {
            if vertex[0] < minima[0] { minima[0] = vertex[0]; }
            if vertex[1] < minima[1] { minima[1] = vertex[1]; }
            if vertex[2] < minima[2] { minima[2] = vertex[2]; }

            if vertex[0] > maxima[0] { maxima[0] = vertex[0]; }
            if vertex[1] > maxima[1] { maxima[1] = vertex[1]; }
            if vertex[2] > maxima[2] { maxima[2] = vertex[2]; }
        }

        for geom::Prim { indices: [a, b, c], .. } in prims {
            let a = vertices[a as usize].pos;
            let b = vertices[b as usize].pos;
            let c = vertices[c as usize].pos;

            extrema_vertex(a, &mut min, &mut max);
            extrema_vertex(b, &mut min, &mut max);
            extrema_vertex(c, &mut min, &mut max);
        }

        Self { min, _p0: 0, max, _p1: 0 }
    }

    fn contains(&self, point: [f32; 3]) -> bool {
        point[0] >= self.min[0] &&
        point[0] <= self.max[0] &&
        point[1] >= self.min[1] &&
        point[1] <= self.max[1] &&
        point[2] >= self.min[2] &&
        point[2] <= self.max[2]
    }
}

#[derive(Clone, Copy)]
pub struct BvhConfig {
    pub eps: f32,
}

static mut BVH_CONFIG: BvhConfig = BvhConfig {
    eps: 0.000002,
};

pub struct BvhIntrs;

impl BvhIntrs {
    const COPY_USAGES: wgpu::BufferUsages = {
        wgpu::BufferUsages::COPY_SRC //
            .union(wgpu::BufferUsages::COPY_DST) //
    };
}

impl super::IntrsHandler for BvhIntrs {
    type Config = BvhConfig;

    fn vars<'a>(
        scene: &crate::scene::Scene, 
        device: &wgpu::Device
    ) -> anyhow::Result<super::IntrsPack<'a>> {
        let aabb = unsafe {
            aabb::Aabb::from_scene(BVH_CONFIG.eps, scene)?
        };

        let data = data::BvhData::new(&aabb);

        let aabb_uniforms = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&data.uniforms),
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
            }
        );

        let aabb_indices = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&data.indices),
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
            }
        );

        let layout = device.create_bind_group_layout(
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
                            ty: wgpu::BufferBindingType::Storage { 
                                read_only: true 
                            },
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        count: None,
                        ty: wgpu::BindingType::Buffer {
                            has_dynamic_offset: false,
                            min_binding_size: None,
                            ty: wgpu::BufferBindingType::Storage { 
                                read_only: true 
                            },
                        },
                    },
                ]
            }
        );

        let group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: aabb_uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: aabb_indices.as_entire_binding(),
                    },
                ],
            }
        );

        Ok({
            super::IntrsPack {
                vars: vec![
                    super::IntrsVar { 
                        var_name: "aabb_uniforms", 
                        var_decl: "var<storage, read>", 
                        var_type: "array<Aabb>", 
                        buffer: aabb_uniforms,
                    },
                    super::IntrsVar { 
                        var_name: "aabb_indices", 
                        var_decl: "var<storage, read>", 
                        var_type: "array<u32>", 
                        buffer: aabb_indices,
                    },
                ],
                group,
                layout,
            }
        })
    }

    fn logic() -> &'static str {"\
        fn intrs(r: Ray, excl: Prim) -> Intrs {
            return intrs_empty();
        }
    "}
    
    fn configure(config: Self::Config) {
        unsafe {
            BVH_CONFIG = config;
        }
    }
}