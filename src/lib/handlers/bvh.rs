// Needed for `device.create_buffer_init`
use wgpu::util::DeviceExt as _;

use once_cell::unsync;

use crate::bvh;

// This stores all configuration options 
// for construction of the BVH and its intersection logic
pub enum BvhConfig {
    Bytes(Vec<u8>),
    Runtime { eps: f32, },
    Default,
}

impl Default for BvhConfig {
    fn default() -> Self { Self::Default }
}

pub struct BvhIntrs {
    pub eps: f32,

    // These members are private, 
    // binaries should access them through BvhConfig
    data: unsync::OnceCell<bvh::BvhData>,
    nodes: unsync::OnceCell<usize>,
}

impl Default for BvhIntrs {
    fn default() -> Self {
        Self { 
            eps: 0.02, 
            data: unsync::OnceCell::new(),
            nodes: unsync::OnceCell::new(),
        }
    }
}

impl BvhIntrs {
    // When reloading scenes, we may want to write into our previous buffers
    const COPY_USAGES: wgpu::BufferUsages = {
        wgpu::BufferUsages::COPY_SRC //
            .union(wgpu::BufferUsages::COPY_DST) //
    };
}

impl super::IntrsHandler for BvhIntrs {
    type Config = BvhConfig;

    fn new(config: Self::Config) -> anyhow::Result<Self> {
        let intrs = match config {
            BvhConfig::Bytes(bytes) => {
                let data = serde_json::from_slice::<bvh::BvhData>(&bytes)?;

                let nodes = data.uniforms.len();

                Self {
                    data: unsync::OnceCell::with_value(data),
                    nodes: unsync::OnceCell::with_value(nodes),
                    ..Default::default()
                }
            },
            BvhConfig::Runtime { eps } => Self {
                eps,
                ..Default::default()
            },
            BvhConfig::Default => Self::default(),
        };

        Ok(intrs)
    }

    fn vars<'a>(
        &self,
        scene: &mut crate::scene::Scene, 
        device: &wgpu::Device
    ) -> super::IntrsPack<'a> {
        // Build the BVH if we haven't already
        let data = self.data.get_or_init(|| {
            let aabb = bvh::Aabb::from_scene(self.eps, scene, 2);

            bvh::BvhData::new(&aabb)
        });

        let bvh::BvhData {
            uniforms,
            indices, ..
        } = data;

        // Set the node count if we haven't already
        self.nodes.get_or_init(|| uniforms.len());

        let aabb_uniforms = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(uniforms),
                usage: wgpu::BufferUsages::STORAGE | Self::COPY_USAGES,
            }
        );

        if let crate::scene::Scene::Active { prims, .. } = scene {
            use std::mem;

            let ordered = indices
                .iter()
                .map(|&idx| prims[idx as usize])
                .collect::<Vec<_>>();

            let _ = mem::replace(prims, ordered);
        }

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

        super::IntrsPack {
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
        }
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

// The intersection logic
const LOGIC: &str = "\
    struct Bounds {
        min: vec3<f32>,
        max: vec3<f32>,
    }

    struct Aabb {
        fst: u32,
        snd: u32,
        item_idx: u32,
        item_count: u32,
        bounds: Bounds,
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
        var t0 = (bb.bounds.min.x - EPS - ray.origin.x) / ray.dir.x;
        var t1 = (bb.bounds.max.x + EPS - ray.origin.x) / ray.dir.x;

        var t_min = min(t0, t1);
        var t_max = max(t0, t1);
        
        t0 = (bb.bounds.min.y - EPS - ray.origin.y) / ray.dir.y;
        t1 = (bb.bounds.max.y + EPS - ray.origin.y) / ray.dir.y;

        t_min = max(t_min, min(min(t0, t1), INF_NEG));
        t_max = min(t_max, max(max(t0, t1), INF_POS));
        
        t0 = (bb.bounds.min.z - EPS - ray.origin.z) / ray.dir.z;
        t1 = (bb.bounds.max.z + EPS - ray.origin.z) / ray.dir.z;

        t_min = max(t_min, min(min(t0, t1), INF_NEG));
        t_max = min(t_max, max(max(t0, t1), INF_POS));

        return (t_min < t_max);
    }

    // TODO: The vector-based collision method is faster,
    // but does not yet incorporate the INF_POS/_NEG testing.
    // Implement the edge case, then switch to this algorithm
    fn collides_wiche(bb: Aabb, ray: Ray) -> bool {
        let t0s = (bb.bounds.min - ray.origin) / ray.dir;
        let t1s = (bb.bounds.max - ray.origin) / ray.dir;

        let t_mins = min(t0s, t1s);
        let t_maxs = max(t0s, t1s);

        let t_min = max(config.t_min, max(t_mins.x, max(t_mins.y, t_mins.z)));
        let t_max = min(config.t_max, min(t_maxs.x, min(t_maxs.y, t_maxs.z)));

        return (t_min < t_max);
    }

    fn intrs_bvh(bb: Aabb, ray: Ray, excl: Prim) -> Intrs {
        var intrs: Intrs = intrs_empty();

        for(var i: u32 = bb.item_idx; i < (bb.item_idx + bb.item_count); i = i + 1u) {
            let prim: Prim = primitives[i];

            let temp: Intrs = intrs_tri(ray, prim);

            if(temp.t < intrs.t) {
                intrs = temp;
            }
        }

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
                if(bb.item_count > 0u) {
                    let temp = intrs_bvh(bb, r, excl);

                    if(temp.t < intrs.t) {
                        intrs = temp;
                    }
                } else {
                    push(&stack_idx, bb.fst);
                    push(&stack_idx, bb.snd);

                    stack_empty = false;
                }
            }
        }

        return intrs;
    }\
";