use std::mem;

use once_cell::unsync;
use wgpu::util::DeviceExt as _;

use crate::bvh;

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Clone, Copy)]
struct RfAabbUniform {
    bounds: [u32; 3],
    tag: u32,
}

pub enum RfBvhConfig {
    Eps(f32),
    Default,
}

impl Default for RfBvhConfig {
    fn default() -> Self { Self::Default }
}

pub struct RfBvhIntrs {
    eps: f32,
    nodes: unsync::OnceCell<usize>,
}

impl Default for RfBvhIntrs {
    fn default() -> Self {
        Self { 
            eps: 0.02, 
            nodes: unsync::OnceCell::new(),
        }
    }
}

impl RfBvhIntrs {
    // When reloading scenes, we may want to write into our previous buffers
    const COPY_USAGES: wgpu::BufferUsages = {
        wgpu::BufferUsages::COPY_SRC //
            .union(wgpu::BufferUsages::COPY_DST) //
    };
}

impl super::IntrsHandler for RfBvhIntrs {
    type Config = RfBvhConfig;

    fn new(config: Self::Config) -> anyhow::Result<Self> 
        where Self: Sized {

        Ok(match config {
            RfBvhConfig::Eps(eps) => Self { eps, ..Default::default() },
            RfBvhConfig::Default => Self::default(),
        })
    }

    fn vars<'a>(
        &self,
        scene: &mut crate::scene::Scene, 
        device: &wgpu::Device,
    ) -> (super::IntrsPack<'a>, super::IntrsStats) {
        let aabb = bvh::Aabb::from_scene(self.eps, scene, 4);

        let data = bvh::BvhData::new(&aabb);

        let bvh::BvhData {
            uniforms,
            indices, ..
        } = data;

        // Set the node count if we haven't already
        self.nodes.get_or_init(|| uniforms.len());

        let mut uniforms_rf = Vec::with_capacity(uniforms.len());

        for uniform in uniforms.iter() {
            let bvh::AabbUniform {
                fst,
                snd,
                item_idx,
                item_count,
                bounds: bvh::Bounds { min, max, .. },
            } = *uniform;

            fn pack(a: f32, b: f32) -> u32 {
                let a = half::f16::from_f32(a);
                let b = half::f16::from_f32(b);

                bytemuck::cast_slice::<half::f16, u32>(&[a, b])[0]
            }

            // If it is a leaf
            if fst == 0 && snd == 0 {
                uniforms_rf.push(RfAabbUniform {
                    bounds: [
                        pack(min[0], max[0]),
                        pack(min[1], max[1]),
                        pack(min[2], max[2]),
                    ],
                    tag: 1 << 31,
                });

                let item_idx = item_idx as usize;
                let item_count = item_count as usize;

                let mut items = indices[item_idx..(item_idx + item_count)]
                    .iter()
                    .map(|&idx| idx as u16)
                    .collect::<Vec<_>>();

                items.extend(std::iter::repeat(0).take(8 - items.len()));

                uniforms_rf.push({
                    bytemuck::cast_slice::<u16, RfAabbUniform>(&items)[0]
                });
            } else { // Internal node
                uniforms_rf.push(RfAabbUniform {
                    bounds: [
                        pack(min[0], max[0]),
                        pack(min[1], max[1]),
                        pack(min[2], max[2]),
                    ],
                    tag: ((fst) << 16) | ((snd) & 0xFFFF),
                });
            };
        }

        for RfAabbUniform { tag, .. } in uniforms_rf.iter_mut() {
            if (*tag >> 31) & 1 == 0 {
                let [
                    mut fst, 
                    mut snd
                ] = bytemuck::cast::<u32, [u16; 2]>(*tag);

                let mut idx = 0;
                let mut offset = 0;
                while idx < fst as usize {
                    if uniforms[idx].item_count > 0 {
                        offset += 1;
                    }
    
                    idx += 1;
                } fst += offset;

                idx = 0; offset = 0;
                while idx < snd as usize {
                    if uniforms[idx].item_count > 0 {
                        offset += 1;
                    }
    
                    idx += 1;
                } snd += offset;

                *tag = bytemuck::cast::<[u16; 2], u32>([fst, snd]);
            }
        }

        let aabb_uniforms = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&uniforms_rf),
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
                ],
            }
        );

        let pack = super::IntrsPack {
            vars: vec![
                super::IntrsVar { 
                    var_name: "aabb_uniforms",
                    var_ty: "array<Aabb>", 
                    buffer: aabb_uniforms,
                    buffer_ty: wgpu::BufferBindingType::Storage { 
                        read_only: true, 
                    },
                },
            ],
            group,
            layout,
        };

        let stats = super::IntrsStats {
            name: "RF-BVH",
            size: mem::size_of::<RfAabbUniform>() * uniforms_rf.len(),
        };

        (pack, stats)
    }

    fn logic(&self) -> &'static str {
        // In the shader code below, this line is incomplete.
        // It needs to be given a type
        const DECL: &str = "var<private> aabb_stack;";

        // IntrsHandler::logic is always called after IntrsHandler::vars,
        // so the diverging case is truly unreachable
        let Some(nodes) = self.nodes.get().copied() else { 
            unreachable!();
        };

        // Perform the replacement
        let mut logic = String::from(LOGIC); logic.insert_str(
            LOGIC.find(DECL).unwrap() + DECL.len() - 1, 
            format!(": array<u32, {nodes}>",).as_str()
        );
        
        // We have to return a static string, so we leak it
        Box::leak(logic.into_boxed_str())
    }
}

#[allow(dead_code)]
fn debug_aabb(data: &bvh::BvhData) {
    fn debug_aabb_inner(data: &bvh::BvhData, curr: usize, indent: usize) {
        let bvh::AabbUniform { 
            fst,
            snd,
            bounds, 
            item_idx, 
            item_count, .. 
        } = data.uniforms[curr];
    
        let [x_min, y_min, z_min] = bounds.min;
        let [x_max, y_max, z_max] = bounds.max;
    
        if data.uniforms[curr].item_count > 0 {
            let item_idx = item_idx as usize;
            let item_count = item_count as usize;
    
            let items = &data.indices[item_idx..(item_idx + item_count)];
    
            let [x_min, y_min, z_min] = bounds.min;
            let [x_max, y_max, z_max] = bounds.max;
    
            println!(
                "{} Leaf [{:.3}, {:.3}, {:.3}] [{:.3}, {:.3}, {:.3}]: {:?}", 
                " ".repeat(indent), 
                x_min, y_min, z_min, 
                x_max, y_max, z_max, 
                items,
            );
        } else {
            println!(
                "{} Node [{:.3}, {:.3}, {:.3}] [{:.3}, {:.3}, {:.3}]", 
                " ".repeat(indent), 
                x_min, y_min, z_min, 
                x_max, y_max, z_max,
            );
    
            debug_aabb_inner(data, fst as usize, indent + 1);
            debug_aabb_inner(data, snd as usize, indent + 1);
        }
    }

    debug_aabb_inner(data, 0, 0);
}

#[allow(dead_code)]
fn debug_rf_aabb(bbs: &[RfAabbUniform]) {
    fn debug_rf_aabb_inner(bbs: &[RfAabbUniform], curr: usize, indent: usize) {
        let RfAabbUniform {
            bounds,
            tag, ..
        } = bbs[curr];
    
        let [x_min, x_max] = bytemuck::cast::<u32, [half::f16; 2]>(bounds[0]);
        let [y_min, y_max] = bytemuck::cast::<u32, [half::f16; 2]>(bounds[1]);
        let [z_min, z_max] = bytemuck::cast::<u32, [half::f16; 2]>(bounds[2]);
    
        if (tag >> 31) & 1 == 0 {
            let [fst, snd] = bytemuck::cast::<u32, [u16; 2]>(tag);
    
            println!(
                "{} Node [{:.3}, {:.3}, {:.3}] [{:.3}, {:.3}, {:.3}]", 
                " ".repeat(indent), 
                x_min, y_min, z_min,
                x_max, y_max, z_max,
            );
    
            debug_rf_aabb_inner(bbs, fst as usize, indent + 1);
            debug_rf_aabb_inner(bbs, snd as usize, indent + 1);
        } else {
            let RfAabbUniform { 
                bounds, 
                tag, ..
            } = bbs[curr + 1];
    
            let mut indices = vec![];
            indices.extend_from_slice(&bytemuck::cast::<u32, [u16; 2]>(bounds[0]));
            indices.extend_from_slice(&bytemuck::cast::<u32, [u16; 2]>(bounds[1]));
            indices.extend_from_slice(&bytemuck::cast::<u32, [u16; 2]>(bounds[2]));
            indices.extend_from_slice(&bytemuck::cast::<u32, [u16; 2]>(tag));
    
            let indices = indices
                .into_iter()
                .filter(|&x| x != 0)
                .collect::<Vec<_>>();
    
            println!(
                "{} Leaf [{:.3}, {:.3}, {:.3}] [{:.3}, {:.3}, {:.3}]: {:?}", 
                " ".repeat(indent), 
                x_min, y_min, z_min, 
                x_max, y_max, z_max, 
                indices,
            );
        }
    }

    debug_rf_aabb_inner(bbs, 0, 0);
}

const LOGIC: &str = "\
    struct Bounds {
        min: vec3<f32>,
        max: vec3<f32>,
    }

    struct Aabb {
        bounds: vec3<u32>,
        tag: u32
    }

    fn intrs_tri(r: Ray, s: Prim) -> Intrs {
        let e1: vec3<f32> = vertices[s.b].pos - vertices[s.a].pos;
        let e2: vec3<f32> = vertices[s.c].pos - vertices[s.a].pos;

        let p: vec3<f32> = cross(r.dir, e2);
        let t: vec3<f32> = r.origin - vertices[s.a].pos;
        let q: vec3<f32> = cross(t, e1);

        let det = dot(e1, p);

        var u: f32 = 0.0;
        var v: f32 = 0.0;
        if(det > config.eps) {
            u = dot(t, p);
            if(u < 0.0 || u > det) { return intrs_empty(); }

            v = dot(r.dir, q);
            if(v < 0.0 || u + v > det) { return intrs_empty(); }
        } else if(det < -1.0 * config.eps) {
            u = dot(t, p);
            if(u > 0.0 || u < det) { return intrs_empty(); }

            v = dot(r.dir, q);
            if(v > 0.0 || u + v < det) { return intrs_empty(); }
        } else {
            return intrs_empty();
        }

        let w: f32 = dot(e2, q) / det;
        
        if(w > config.t_max || w < config.t_min) {
            return intrs_empty();
        } else {
            return Intrs(s, w);
        }
    }

    const INF_POS: f32 = 0x1.p+38f;
    const INF_NEG: f32 = -1.0 * INF_POS;

    // Wobble for the intersection test below
    const EPS: f32 = 0.000002;

    fn collides(bb: Aabb, ray: Ray) -> bool {
        let a: vec2<f32> = unpack2x16float(bb.bounds.x);
        let b: vec2<f32> = unpack2x16float(bb.bounds.y);
        let c: vec2<f32> = unpack2x16float(bb.bounds.z);

        let minima: vec3<f32> = vec3<f32>(a.x, b.x, c.x);
        let maxima: vec3<f32> = vec3<f32>(a.y, b.y, c.y);
        
        var t0 = (minima.x - EPS - ray.origin.x) / ray.dir.x;
        var t1 = (maxima.x + EPS - ray.origin.x) / ray.dir.x;

        var t_min = min(t0, t1);
        var t_max = max(t0, t1);
        
        t0 = (minima.y - EPS - ray.origin.y) / ray.dir.y;
        t1 = (maxima.y + EPS - ray.origin.y) / ray.dir.y;

        t_min = max(t_min, min(min(t0, t1), INF_NEG));
        t_max = min(t_max, max(max(t0, t1), INF_POS));
        
        t0 = (minima.z - EPS - ray.origin.z) / ray.dir.z;
        t1 = (maxima.z + EPS - ray.origin.z) / ray.dir.z;

        t_min = max(t_min, min(min(t0, t1), INF_NEG));
        t_max = min(t_max, max(max(t0, t1), INF_POS));

        return (t_min < t_max);
    }

    fn intrs_bvh_helper(idx: u32, ray: Ray, curr: Intrs) -> Intrs {
        if(idx != 0u) {
            let prim: Prim = primitives[idx];

            let temp: Intrs = intrs_tri(ray, prim);

            if(temp.t < curr.t) {
                return temp;
            }
        }
        
        return curr;
    }

    fn intrs_bvh(bb: Aabb, ray: Ray, excl: Prim) -> Intrs {
        var intrs: Intrs = intrs_empty();

        var t = bb.bounds.x & 0xFFFF;
        intrs = intrs_bvh_helper(t, ray, intrs);
        t = (bb.bounds.x >> 16) & 0xFFFF;
        intrs = intrs_bvh_helper(t, ray, intrs);

        t = bb.bounds.y & 0xFFFF;
        intrs = intrs_bvh_helper(t, ray, intrs);
        t = (bb.bounds.y >> 16) & 0xFFFF;
        intrs = intrs_bvh_helper(t, ray, intrs);

        t = bb.bounds.z & 0xFFFF;
        intrs = intrs_bvh_helper(t, ray, intrs);
        t = (bb.bounds.z >> 16) & 0xFFFF;
        intrs = intrs_bvh_helper(t, ray, intrs);

        return intrs;
    }

    // NOTE: The type is specified by BvhIntrs::logic
    var<private> aabb_stack;

    fn pop(idx: ptr<function, u32>, empty: ptr<function, bool>) -> u32 {
        if(*idx == 1u) {
            *empty = true;
        }

        *idx = *idx - 1u;

        return aabb_stack[*idx];
    }

    fn push(idx: ptr<function, u32>, bb: u32) {
        aabb_stack[*idx] = bb;

        *idx = *idx + 1u;
    }

    fn intrs(r: Ray, excl: Prim) -> Intrs {
        var stack_idx = 0u;
        var stack_empty = false;

        push(&stack_idx, 0u);

        var intrs = intrs_empty();

        while(!stack_empty) {
            let bb_idx = pop(&stack_idx, &stack_empty);
            let bb = aabb_uniforms[bb_idx];

            if(collides(bb, r)) {
                if((bb.tag >> 31 & 1) == 1u) {
                    let temp = intrs_bvh(aabb_uniforms[bb_idx + 1u], r, excl);

                    if(temp.t < intrs.t) {
                        intrs = temp;
                    }
                } else {
                    let fst: u32 = bb.tag & 0xFFFF;
                    push(&stack_idx, fst);

                    let snd: u32 = (bb.tag >> 16) & 0xFFFF;
                    push(&stack_idx, snd);

                    stack_empty = false;
                }
            }
        }

        return intrs;
    }\
";