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

impl<'a> IntrsVar<'a> {
    pub fn destroy(&self) {
        self.buffer.destroy();
    }
}

#[derive(Debug)]
pub struct IntrsPack<'a> {
    pub vars: Vec<IntrsVar<'a>>,
    pub group: wgpu::BindGroup,
    pub layout: wgpu::BindGroupLayout,
}

impl<'a> IntrsPack<'a> {
    pub fn destroy(&self) {
        for var in self.vars.iter() {
            var.destroy();
        }
    }
}

pub trait IntrsHandler {
    type Config: Default;

    fn new(config: Self::Config) -> anyhow::Result<Self> 
        where Self: Sized;

    // Builds all the requisite buffers and groups
    fn vars<'a>(
        &self,
        scene: &scene::Scene, 
        device: &wgpu::Device,
    ) -> IntrsPack<'a>;

    // Contains all of the intersection logic
    fn logic(&self) -> &'static str;
}