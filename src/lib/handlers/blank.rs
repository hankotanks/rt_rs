use crate::scene;

// This handler just renders a blank screen
// Used to test benchmarking baseline
pub struct BlankIntrs;

impl super::IntrsHandler for BlankIntrs {
    type Config = ();

    fn new(_config: ()) -> anyhow::Result<Self> { Ok(Self) }
    
    fn vars<'a>(
        &self,
        _scene: &scene::Scene, device: &wgpu::Device,
    ) -> super::IntrsPack<'a> {
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