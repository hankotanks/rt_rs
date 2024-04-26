// TODO: This should not be public
pub mod aabb;

struct BvhHandler;

impl super::IntrsHandler for BvhHandler {
    fn vars<'a>(
        scene: &crate::scene::Scene, 
        device: &wgpu::Device
    ) -> super::IntrsPack<'a> {
        super::basic::BasicIntrs::vars(scene, device)
    }

    fn logic() -> &'static str {"\
        fn intrs(r: Ray, excl: Prim) -> Intrs {
            return intrs_empty();
        }
    "}
}