use crate::scene;

// This handler just renders a blank screen
// Used to test benchmarking baseline
pub struct BlankIntrs;

impl super::IntrsHandler for BlankIntrs {
    type Config = ();

    fn new(_config: ()) -> anyhow::Result<Self> { Ok(Self) }
    
    fn vars<'a>(
        &self,
        _scene: &mut scene::Scene, device: &wgpu::Device,
    ) -> (super::IntrsPack<'a>, super::IntrsStats) {
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

        let pack = super::IntrsPack {
            vars: Vec::with_capacity(0),
            group,
            layout,
        };

        let stats = super::IntrsStats { 
            name: "Blank",
            size: 0,
        };

        (pack, stats)
    }

    fn logic(&self) -> &'static str {
        "fn intrs(r: Ray, excl: Prim) -> Intrs { return intrs_empty(); }"
    }
}