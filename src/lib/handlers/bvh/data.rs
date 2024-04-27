#[repr(C)]
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct AabbUniform {
    pub fst: u32,
    pub snd: u32,
    pub bounds: super::Bounds,
    pub item_idx: u32,
    pub item_count: u32,
}

#[derive(Default)]
pub struct BvhData {
    pub uniforms: Vec<AabbUniform>,
    pub indices: Vec<u32>,
}

impl BvhData {
    pub fn new(aabb: &super::Aabb) -> Self {
        let mut data = Self::default();

        fn into_aabb_uniform(
            data: &mut BvhData,
            aabb: &super::Aabb
        ) {
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
                into_aabb_uniform(data, fst);
        
                data.uniforms[uniform].fst = data.uniforms.len() as u32 - 1;
            }
        
            if let Some(snd) = aabb.snd.get() {
                into_aabb_uniform(data, snd);
        
                data.uniforms[uniform].snd = data.uniforms.len() as u32 - 1;
            }
        }
        
        into_aabb_uniform(&mut data, aabb);

        data
    }
}
