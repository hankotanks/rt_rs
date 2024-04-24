#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Clone, Copy)]
#[derive(Debug)]
pub struct Light {
    pub pos: [f32; 3],
    pub strength: f32,
}

impl<'de> serde::Deserialize<'de> for Light {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        struct Intermediate {
            pos: Vec<f32>,
            strength: f32,
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

        Ok(Self {
            pos,
            strength: intermediate.strength,
        })
    }
}