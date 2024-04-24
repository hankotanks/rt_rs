pub mod camera;

use crate::geom;
use crate::geom::light as light;

pub struct ScenePack {
    pub camera_buffer: wgpu::Buffer,
    pub buffers: Vec<wgpu::Buffer>,
    pub bg: wgpu::BindGroup,
    pub bg_layout: wgpu::BindGroupLayout,
}

#[derive(serde::Deserialize)]
pub struct Scene {
    pub camera: camera::CameraUniform,
    pub camera_controller: camera::CameraController,
    pub prims: Vec<geom::Prim>,
    pub vertices: Vec<geom::PrimVertex>,
    pub lights: Vec<light::Light>,
    pub materials: Vec<geom::PrimMat>,
}

impl Scene {
    const COPY_USAGES: wgpu::BufferUsages = {
        wgpu::BufferUsages::COPY_SRC //
            .union(wgpu::BufferUsages::COPY_DST) //
    };

    pub fn pack(&self, device: &wgpu::Device) -> ScenePack {
        use wgpu::util::DeviceExt as _;

        let Scene { 
            camera,
            prims, 
            vertices,
            lights, 
            materials, .. 
        } = self;

        // Separate the contents out to prevent premature drop
        let camera_buffer_contents = [*camera];

        // Get a muckable CameraUniform and construct buffer
        let camera_buffer_descriptor = wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&camera_buffer_contents[..]),
            usage: wgpu::BufferUsages::UNIFORM | Self::COPY_USAGES,
        };

        let camera_buffer = device.create_buffer_init({
            &camera_buffer_descriptor
        });

        // The first primitive acts as a 'null'
        let mut primitives = vec![
            geom::Prim { indices: [0; 3], material: 0 }
        ];
        
        // Then we add all the others
        primitives.extend(prims.iter().copied());

        //
        // group(2) Scene Buffer and Groups

        // 0: 'camera'
        // 1: 'primitives'
        // 2: 'vertices'
        // 3: 'lights'
        // 4: 'materials'

        // NOTE: Gotta keep camera distinct,
        // because later we need the actual buffer.
        // Keep an eye out for whether a given variable below actually
        // contains it, or if it needs to be added back before mapping
        let buffer_descriptors = &[
            &camera_buffer_descriptor,
            &wgpu::util::BufferInitDescriptor {
                label: None,
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
                contents: bytemuck::cast_slice(primitives.as_slice()),
            },
            &wgpu::util::BufferInitDescriptor {
                label: None,
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
                contents: bytemuck::cast_slice(vertices.as_slice()),
            },
            &wgpu::util::BufferInitDescriptor {
                label: None,
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
                contents: bytemuck::cast_slice(lights),
            },
            &wgpu::util::BufferInitDescriptor {
                label: None,
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
                contents: bytemuck::cast_slice(materials),
            }
        ];

        // Use the descriptors to create buffers
        let buffers: Vec<wgpu::Buffer> = buffer_descriptors[1..]
            .iter()
            .map(|desc| device.create_buffer_init(desc))
            .collect();

        // Construct the layout
        let bg_layout_entries = buffer_descriptors
            .iter()
            .enumerate()
            .map(|(binding, desc)| (binding as u32, desc))
            .map(|(binding, wgpu::util::BufferInitDescriptor { usage, .. })| {
                let ty = if usage.contains(wgpu::BufferUsages::UNIFORM) {
                    wgpu::BufferBindingType::Uniform
                } else if usage.contains(wgpu::BufferUsages::STORAGE) {
                    wgpu::BufferBindingType::Storage { read_only: true, }
                } else {
                    unreachable!();
                };

                wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        has_dynamic_offset: false,
                        min_binding_size: None,
                        ty,
                    }
                }
            }).collect::<Vec<wgpu::BindGroupLayoutEntry>>();

        let bg_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: bg_layout_entries.as_slice(),
            }
        );

        // Construct the bind group
        let bg_entries = [
            &[&camera_buffer][..], 
            buffers.iter().collect::<Vec<_>>().as_slice()
        ].concat();

        let bg_entries: Vec<wgpu::BindGroupEntry> = bg_entries
            .iter()
            .enumerate()
            .map(|(binding, buffer)| (binding as u32, buffer))
            .map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }).collect();

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bg_layout,
            entries: bg_entries.as_slice(),
        });

        // Pack and return
        ScenePack { 
            camera_buffer, 
            buffers,
            bg,
            bg_layout, 
        }
    }

    pub fn add_mesh(
        &mut self, 
        obj: wavefront::Obj,
        material: u32,
    ) {
        use crate::geom::v3::V3Ops as _;

        let vertices = obj.positions().to_vec();
    
        let mut normals = vec![vec![]; vertices.len()];
    
        let mut prims = vec![];
        for [
            (pa, na, idx_a), 
            (pb, nb, idx_b), 
            (pc, nc, idx_c)
        ] in obj.triangles().map(|[a, b, c]| [
                (a.position(), a.normal(), a.position_index()), 
                (b.position(), b.normal(), b.position_index()), 
                (c.position(), c.normal(), c.position_index()),
        ]) {
            let ab = pb.sub(pa);
            let ac = pc.sub(pa);
    
            let normal = ab.cross(ac).normalize();
            
            normals[idx_a].push(match na {
                Some(normal) => normal,
                None => normal.scale(pa.angle(pb, pc)),
            });

            normals[idx_b].push(match nb {
                Some(normal) => normal,
                None => normal.scale(pb.angle(pc, pa)),
            });

            normals[idx_c].push(match nc {
                Some(normal) => normal,
                None => normal.scale(pc.angle(pa, pb)),
            });
    
            prims.push(geom::Prim { 
                indices: [
                    (idx_a + self.vertices.len()) as u32, 
                    (idx_b + self.vertices.len()) as u32,
                    (idx_c + self.vertices.len()) as u32
                ],
                material,
            });
        }
    
        let normals = normals.into_iter().map(|normal| {
            normal.into_iter().fold([0.; 3], |n, c| n.add(c)).normalize()
        }).collect::<Vec<_>>();

        self.vertices.extend({
            vertices.into_iter().enumerate().map(|(idx, pos)| {
                geom::PrimVertex::new(pos, normals[idx])
            })
        });

        self.prims.append(&mut prims);
    }
}