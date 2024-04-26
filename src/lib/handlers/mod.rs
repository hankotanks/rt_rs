mod basic;
pub use basic::BasicIntrs;

mod bvh;
// TODO: Remove Aabb from this export once
// The testing binary `bb` is deleted
pub use bvh::{BvhIntrs, BvhConfig, Aabb};

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

pub trait IntrsHandler {
    type Config;

    // Builds all the requisite buffers and groups
    fn vars<'a>(
        scene: &scene::Scene, 
        device: &wgpu::Device,
    ) -> anyhow::Result<IntrsPack<'a>>;

    // Contains all of the intersection logic
    fn logic() -> &'static str;

    fn set_data(config: Self::Config);
    fn get_data() -> Self::Config;
}