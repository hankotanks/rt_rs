use std::{borrow, io};

use crate::handlers;

pub enum ShaderStage<'a, 'b: 'a> {
    Compute { wg: u32, pack: &'a handlers::IntrsPack<'b>, },
    Render,
}

pub fn source<'a, 'b: 'a, H: handlers::IntrsHandler>(
    stage: ShaderStage<'a, 'b>,
) -> anyhow::Result<wgpu::ShaderSource<'static>> {
    
    // Helper function to find index of @compute declarative
    fn main_cs_idx(source: &str) -> anyhow::Result<usize> {
        let result = source.find("@compute").ok_or({
            #[allow(unused_parens)]
            io::Error::new(io::ErrorKind::InvalidData, ("\
                Compute shader had no entry point [@compute], \
                so the IntrsHandler's logic could not be inserted.\
            "))
        })?;

        Ok(result)
    }

    let source = match stage {
        ShaderStage::Render => { //
            include_str!("render.wgsl").into()
        },
        ShaderStage::Compute { wg, pack, } => {
            let source: &'static str = include_str!("compute.wgsl");

            let source = source.replace(
                "@workgroup_size(16, 16, 1)", 
                &format!("@workgroup_size({}, {}, 1)", wg, wg)
            );

            // No more replacements from here on out
            let mut source = source;

            // Each group contains its own bindings
            let handlers::IntrsPack { vars, .. } = pack;

            // Construct and insert all binding statements
            for (binding, var) in vars.iter().enumerate() {
                let handlers::IntrsVar { 
                    var_name, 
                    var_decl, 
                    var_type, .. 
                } = var;
                
                // NOTE: group(3) is hard-coded
                // See the same behavior in `State::update`
                let binding = format!("
                    @group(3) @binding({binding})
                    {var_decl} {var_name}: {var_type};
                ");

                // The insertion index changes each time
                source.insert_str(main_cs_idx(&source)?, &binding);
            }

            // Add the intersection logic
            let source = source.replace(
                "fn intrs(ray: Ray, excl: Prim) -> Intrs { return intrs_empty(); }", 
                H::logic()
            );

            borrow::Cow::Borrowed({
                Box::leak(source.into_boxed_str())
            })
        },
    };

    Ok(wgpu::ShaderSource::Wgsl(source))
}