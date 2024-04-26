// TODO: This should not be public
mod aabb;
pub use aabb::Aabb;

#[derive(Clone, Copy)]
pub struct BvhConfig {
    pub eps: f32,
}

static mut BVH_CONFIG: BvhConfig = BvhConfig {
    eps: 0.000002,
};

pub struct BvhIntrs;

impl super::IntrsHandler for BvhIntrs {
    type Config = BvhConfig;

    fn vars<'a>(
        scene: &crate::scene::Scene, 
        device: &wgpu::Device
    ) -> anyhow::Result<super::IntrsPack<'a>> {
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[derive(bytemuck::Pod, bytemuck::Zeroable)]
        struct AabbUniform {
            fst: u32,
            snd: u32,
            bounds: aabb::Bounds,
            item_idx: u32,
            item_count: u32,
        }

        let _bb = unsafe {
            aabb::Aabb::from_scene(BVH_CONFIG.eps, scene)?
        };

        super::basic::BasicIntrs::vars(scene, device)
    }

    fn logic() -> &'static str {"\
        fn intrs(r: Ray, excl: Prim) -> Intrs {
            return intrs_empty();
        }
    "}
    
    fn set_data(config: Self::Config) {
        unsafe {
            BVH_CONFIG = config;
        }
    }
    
    fn get_data() -> Self::Config {
        unsafe { 
            BVH_CONFIG 
        }
    }
}