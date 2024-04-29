use crate::scene;

// This handler just renders a blank screen
// Used to test benchmarking baseline
#[derive(Clone, Copy)]
pub struct BlankIntrs;

impl super::IntrsHandler for BlankIntrs {
    type Config = ();
    
    fn vars<'a>(
        _scene: &scene::Scene, device: &wgpu::Device,
    ) -> anyhow::Result<super::IntrsPack<'a>> {
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

        Ok(super::IntrsPack {
            vars: Vec::with_capacity(0),
            group,
            layout,
        })
    }

    fn logic() -> &'static str {
        "fn intrs(r: Ray, excl: Prim) -> Intrs { return intrs_empty(); }"
    }
    
    fn configure(_config: Self::Config) { /*  */ }
}