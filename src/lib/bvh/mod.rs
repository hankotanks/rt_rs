mod aabb;

pub use aabb::Aabb;

// The Aabb tree gets rendered down into an array of AabbUniform structs
// It's placed at the module root to avoid importing items from siblings
#[repr(C)]
#[derive(Clone, Copy)]
#[derive(serde::Deserialize, serde::Serialize)]
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
#[derive(Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
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