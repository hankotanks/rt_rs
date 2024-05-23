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
}

impl Default for RfBvhIntrs {
    fn default() -> Self {
        Self { eps: 0.02, }
    }
}

impl super::IntrsHandler for RfBvhIntrs {
    type Config = RfBvhConfig;

    fn new(config: Self::Config) -> anyhow::Result<Self> 
        where Self: Sized {

        Ok(match config {
            RfBvhConfig::Eps(eps) => Self { eps },
            RfBvhConfig::Default => Self::default(),
        })
    }

    fn vars<'a>(
        &self,
        scene: &mut crate::scene::Scene, 
        device: &wgpu::Device,
    ) -> super::IntrsPack<'a> {
        let aabb = bvh::Aabb::from_scene(self.eps, scene, 4);

        let data = bvh::BvhData::new(&aabb);

        let bvh::BvhData {
            uniforms,
            indices, ..
        } = data;

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

        let layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[]
            }
        );

        let group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[],
            }
        );

        super::IntrsPack {
            vars: Vec::with_capacity(0),
            group,
            layout,
        }
    }

    fn logic(&self) -> &'static str {
        "fn intrs(r: Ray, excl: Prim) -> Intrs { return intrs_empty(); }"
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