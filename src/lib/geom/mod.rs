pub mod light;
pub mod v3;

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Clone, Copy)]
pub struct Prim {
    pub indices: [u32; 3],
    pub material: u32,
}

impl<'de> serde::Deserialize<'de> for Prim {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        struct Intermediate {
            indices: Vec<u32>,
            material: u32,
        }

        let intermediate = Intermediate::deserialize(deserializer)?;

        let indices = match intermediate.indices.len() {
            3 => {
                let mut indices = [0; 3];

                indices.copy_from_slice(&intermediate.indices);
                indices
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.indices.len(), 
                    &"an array of len 3",
                ));
            }
        };

        Ok(Self {
            indices,
            material: intermediate.material,
        })
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Clone, Copy)]
pub struct PrimVertex {
    pub pos: [f32; 3],
    _p0: u32,
    pub normal: [f32; 3],
    _p1: u32,
}

impl PrimVertex {
    pub const fn new(pos: [f32; 3], normal: [f32; 3]) -> Self {
        Self {
            pos, 
            _p0: 0,
            normal, 
            _p1: 0,
        }
    }
}

impl<'de> serde::Deserialize<'de> for PrimVertex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        struct Intermediate {
            pos: Vec<f32>,
            normal: Vec<f32>,
        }

        let intermediate = Intermediate::deserialize(deserializer)?;

        let pos = match intermediate.pos.len() {
            3 => {
                let mut pos = [0.; 3];

                pos.copy_from_slice(&intermediate.pos);
                pos
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.pos.len(), 
                    &"an array of len 3",
                ));
            }
        };

        let normal = match intermediate.normal.len() {
            3 => {
                let mut normal = [0.; 3];

                normal.copy_from_slice(&intermediate.normal);
                normal
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.pos.len(), 
                    &"an array of len 3",
                ));
            }
        };

        Ok(Self::new(pos, normal))
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Clone, Copy)]
pub struct PrimMat {
    pub color: [f32; 3],
    _p0: u32,
    pub albedo: [f32; 3],
    pub spec: f32,
}

impl PrimMat {
    pub const fn new(color: [f32; 3], albedo: [f32; 3], spec: f32) -> Self {
        Self {
            color,
            _p0: 0,
            albedo,
            spec
        }
    }
}

impl<'de> serde::Deserialize<'de> for PrimMat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        struct Intermediate {
            color: Vec<f32>,
            albedo: Vec<f32>,
            spec: f32,
        }

        let intermediate = Intermediate::deserialize(deserializer)?;

        // TODO: This can be factored out, 
        // it's used everywhere to deserialize [f32; 3]
        let color = match intermediate.color.len() {
            3 => {
                let mut color = [0.; 3];

                color.copy_from_slice(&intermediate.color);
                color
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.color.len(), 
                    &"an array of len 3",
                ));
            }
        };

        let albedo = match intermediate.albedo.len() {
            3 => {
                let mut albedo = [0.; 3];

                albedo.copy_from_slice(&intermediate.albedo);
                albedo
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.albedo.len(), 
                    &"an array of len 3",
                ));
            }
        };

        Ok(Self::new(color, albedo, intermediate.spec))
    }
}