mod basic;
pub use basic::BasicIntrs;

mod bvh;
pub use bvh::{BvhIntrs, BvhConfig};

mod blank;
// NOTE: Dummy intersection handler used for benchmarking
pub use blank::BlankIntrs;

use crate::scene;

#[derive(Debug)]
pub struct IntrsVar<'a> {
    pub var_name: &'a str,
    pub var_decl: &'a str,
    pub var_type: &'a str,
    pub buffer: wgpu::Buffer,
}

#[derive(Debug)]
pub struct IntrsPack<'a> {
    pub vars: Vec<IntrsVar<'a>>,
    pub group: wgpu::BindGroup,
    pub layout: wgpu::BindGroupLayout,
}

pub trait IntrsHandler: Copy {
    type Config;

    // Builds all the requisite buffers and groups
    fn vars<'a>(
        scene: &scene::Scene, 
        device: &wgpu::Device,
    ) -> anyhow::Result<IntrsPack<'a>>;

    // Contains all of the intersection logic
    fn logic() -> &'static str;

    fn configure(config: Self::Config);
}