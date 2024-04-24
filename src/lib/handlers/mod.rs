pub mod basic;

use crate::scene;

pub struct IntrsVar<'a> {
    pub var_name: &'a str,
    pub var_decl: &'a str,
    pub var_type: &'a str,
    pub buffer: wgpu::Buffer,
}

pub struct IntrsPack<'a> {
    pub vars: Vec<IntrsVar<'a>>,
    pub group: wgpu::BindGroup,
    pub layout: wgpu::BindGroupLayout,
}

pub trait IntrsHandler {
    // Builds all the requisite buffers and groups
    fn vars<'a>(
        scene: &scene::Scene, 
        device: &wgpu::Device) -> IntrsPack<'a>;

    // Contains all of the intersection logic
    fn logic() -> &'static str;
}