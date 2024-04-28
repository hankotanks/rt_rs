mod aabb;

// Needed for `device.create_buffer_init`
use wgpu::util::DeviceExt as _;

// This stores all configuration options 
// for construction of the BVH and its intersection logic
#[derive(Clone, Copy)]
pub struct BvhConfig {
    pub eps: f32,
}

// Since BvhIntrs is never actually initialized, we keep it in a global
// This is safe, because it is only set once -> before any reads can occur
static mut CONFIG: BvhConfig = BvhConfig {
    eps: 0.02,
};

// This tracks the size of the tree
// Populated after BvhIntrs::vars is called in State::new
static mut NODES: usize = 0;

// The dummy struct that the handler methods are implemented on
pub struct BvhIntrs;

impl BvhIntrs {
    // When reloading scenes, we may want to write into our previous buffers
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
            aabb::Aabb::from_scene(CONFIG.eps, scene)?
        };

        let data = BvhData::new(&aabb);

        unsafe {
            // This is set before BvhIntrs::logic is called,
            // enabling the stack to be sized correctly
            NODES = data.uniforms.len();
        }

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
                    // TODO: 2 future changes
                    // - `var_decl` can be a vec of terms: ["storage", "read"]
                    // - Should add a field with struct declarations that are
                    //   relevant to the handler's logic
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

    fn logic() -> &'static str {
        let eps = unsafe {
            format!("{}", CONFIG.eps)
        };

        let nodes = unsafe {
            format!("{}", NODES)
        };

        let logic = LOGIC
            .replace("<NODES>", &nodes)
            .replace("<EPS>", &eps);

        Box::leak(logic.into_boxed_str())
    }
    
    fn configure(config: Self::Config) {
        unsafe {
            CONFIG = config;
        }
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

    fn intrs_tri(ray: Ray, tri: Prim) -> Intrs {
        let a = vertices[tri.a].pos;
        let b = vertices[tri.b].pos;
        let c = vertices[tri.c].pos;

        let ab = b - a;
        let ac = c - a;

        let p = cross(ray.dir, ac);
        let det = dot(ab, p);

        if (det < config.eps) { return intrs_empty(); }

        if(abs(det) < config.eps) { return intrs_empty(); }

        let det_inv = 1.0 / det;

        let t = ray.origin - a;
        let u = dot(t, p) * det_inv;
        if(u < 0.0 || u > 1.0) { return intrs_empty(); }

        let q = cross(t, ab);
        let v = dot(ray.dir, q) * det_inv;
        if(v < 0.0 || (u + v) > 1.0) { return intrs_empty(); }

        let w = dot(ac, q) * det_inv;

        return Intrs(tri, w);
    }

    fn collides(bb: Aabb, ray: Ray) -> bool {
        var t_min: f32 = bitcast<f32>(0x7F7FFFFF) * -1.0;
        var t_max: f32 = bitcast<f32>(0x7F7FFFFF);

        var t0 = (bb.bounds.min.x - <EPS> - ray.origin.x) / ray.dir.x;
        var t1 = (bb.bounds.max.x + <EPS> - ray.origin.x) / ray.dir.x;

        t_min = max(t_min, min(t0, t1));
        t_max = min(t_max, max(t0, t1));
        
        t0 = (bb.bounds.min.y - <EPS> - ray.origin.y) / ray.dir.y;
        t1 = (bb.bounds.max.y + <EPS> - ray.origin.y) / ray.dir.y;

        t_min = max(t_min, min(t0, t1));
        t_max = min(t_max, max(t0, t1));
        
        t0 = (bb.bounds.min.z - <EPS> - ray.origin.z) / ray.dir.z;
        t1 = (bb.bounds.max.z + <EPS> - ray.origin.z) / ray.dir.z;

        t_min = max(t_min, min(t0, t1));
        t_max = min(t_max, max(t0, t1));

        return (t_min < t_max);
    }

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
            let prim: Prim = primitives[aabb_indices[i]];

            let temp: Intrs = intrs_tri(ray, prim);

            if(temp.t < intrs.t) {
                intrs = temp;
            }
        }

        return intrs;
    }

    fn intrs_bvh_smits(bb: Aabb, ray: Ray, excl: Prim) -> Intrs {
        // https://people.csail.mit.edu/amy/papers/box-jgt.pdf
    }

    var<private> aabb_stack: array<u32, <NODES>>;

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

            if(collides_wiche(bb, r)) {
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

// The Aabb tree gets rendered down into an array of AabbUniform structs
// It's placed at the module root to avoid importing items from siblings
#[repr(C)]
#[derive(Clone, Copy)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct AabbUniform {
    pub fst: u32,
    pub snd: u32,
    pub item_idx: u32,
    pub item_count: u32,
    pub bounds: aabb::Bounds,
}

// I've factored out the process of making the Aabb tree compute-friendly
// for simplicity's sake
#[derive(Default)]
pub struct BvhData {
    pub uniforms: Vec<AabbUniform>,
    pub indices: Vec<u32>,
}

impl BvhData {
    // Construct the shader data from the root node of the tree
    pub fn new(aabb: &aabb::Aabb) -> Self {
        let mut data = Self::default();

        fn into_aabb_uniform(
            data: &mut BvhData,
            aabb: &aabb::Aabb
        ) -> u32 {
            let uniform = data.uniforms.len();
        
            data.uniforms.push(AabbUniform {
                fst: 0,
                snd: 0,
                bounds: aabb.bounds,
                item_idx: data.indices.len() as u32,
                item_count: aabb.items.len() as u32,
            });
        
            data.indices.extend(aabb.items.iter().map(|&i| i as u32));
        
            if let Some(fst) = aabb.fst.get() {
                data.uniforms[uniform].fst = into_aabb_uniform(data, fst);
            }
        
            if let Some(snd) = aabb.snd.get() {
                data.uniforms[uniform].snd = into_aabb_uniform(data, snd);
            }

            uniform as u32
        }
        
        into_aabb_uniform(&mut data, aabb);

        data
    }
}