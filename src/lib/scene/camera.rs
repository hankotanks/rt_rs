use winit::{dpi, event, keyboard};

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Clone, Copy)]
#[derive(serde::Serialize)]
#[derive(Debug)]
pub struct CameraUniform {
    pub pos: [f32; 3],
    #[serde(skip_serializing)]
    _p0: u32,
    pub at: [f32; 3],
    #[serde(skip_serializing)]
    _p1: u32,
}

impl CameraUniform {
    pub const fn new(pos: [f32; 3], at: [f32; 3]) -> Self {
        Self {
            pos,
            _p0: 0,
            at,
            _p1: 0,
        }
    }
}

impl<'de> serde::Deserialize<'de> for CameraUniform {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        struct Intermediate {
            pos: Vec<f32>,
            at: Vec<f32>,
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

        let at = match intermediate.at.len() {
            3 => {
                let mut at = [0.; 3];

                at.copy_from_slice(&intermediate.at);
                at
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.at.len(), 
                    &"an array of len 3",
                ));
            }
        };

        Ok(Self::new(pos, at))
    }
}

#[derive(Clone, Copy)]
#[derive(Debug)]
pub enum CameraController {
    Orbit { left: bool, right: bool, scroll: i32, },
    Fixed,
}

impl<'de> serde::Deserialize<'de> for CameraController {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where  D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        enum Intermediate {
            Orbit,
            Fixed,
        }

        #[allow(clippy::from_over_into)]
        impl Into<CameraController> for Intermediate {
            fn into(self) -> CameraController {
                match self {
                    Intermediate::Orbit => //
                        CameraController::Orbit { 
                            left: false, 
                            right: false, 
                            scroll: 0, 
                        },
                    Intermediate::Fixed => //
                        CameraController::Fixed,
                }
            }
        }

        Ok(Intermediate::deserialize(deserializer)?.into())
    }
}

impl serde::Serialize for CameraController {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer {

        #[derive(serde::Serialize)]
        enum Intermediate {
            Orbit,
            Fixed,
        }

        impl From<CameraController> for Intermediate {
            fn from(value: CameraController) -> Self {
                match value {
                    CameraController::Orbit { .. } => Intermediate::Orbit,
                    CameraController::Fixed => Intermediate::Fixed,
                }
            }
        }

        Into::<Intermediate>::into(*self).serialize(serializer)
    }
}

impl CameraController {
    #[allow(dead_code)]
    pub fn handle_event(&mut self, event: &event::WindowEvent) -> bool {
        // The fixed camera never consumes an event
        let Self::Orbit {
            left, right, scroll, ..
        } = self else { return false; };

        match event {
            event::WindowEvent::KeyboardInput {
                event: event::KeyEvent {
                    logical_key: keyboard::Key::Named(key),
                    state, ..
                }, ..
            } => {
                let pressed = matches!(state, event::ElementState::Pressed);

                let mut handled = true;
                match *key {
                    keyboard::NamedKey::ArrowLeft => *left = pressed,
                    keyboard::NamedKey::ArrowRight => *right = pressed,
                    _ => handled = false,
                }
    
                handled
            },
            event::WindowEvent::MouseWheel { 
                delta: event::MouseScrollDelta::PixelDelta(
                    dpi::PhysicalPosition { y, .. }
                ), .. 
            } => {
                *scroll = match y.signum() as i32 { -1 => -1, 1 => 1, _ => 0, };

                true
            },
            _ => false
        }
    }

    #[allow(dead_code)]
    pub fn update(&mut self, uniform: &mut CameraUniform) -> bool {
        use crate::geom::v3::V3Ops as _;

        const SPEED: f32 = 0.05;

        let Self::Orbit { 
            left, right, scroll, ..
        } = self else { return false; };

        fn orbit(uni: &mut CameraUniform, mult: f32) {
            let x = uni.pos[0] - uni.at[0];
            let z = uni.pos[2] - uni.at[2];

            let theta = z.atan2(x) + 0.0314 * mult;
            
            let mag = (x * x + z * z).sqrt();

            let x = uni.at[0] + mag * theta.cos();
            let z = uni.at[2] + mag * theta.sin();

            uni.pos = [x, uni.pos[1], z];
        }

        if *left {
            orbit(uniform, 1.);

            return true;
        }

        if *right {
            orbit(uniform, -1.);

            return true;
        }

        match scroll {
            -1 => {
                let v = uniform.at.sub(uniform.pos);

                uniform.pos = uniform.pos.sub(v.normalize().scale(SPEED));

                *scroll = 0;

                return true;
            },
            1 => {
                let v = uniform.at.sub(uniform.pos);

                let pos = uniform.pos.add(v.normalize().scale(SPEED));

                let dist = uniform.at.sub(pos).mag();
                if dist.abs() > 0. && dist.signum() > -0. {
                    uniform.pos = pos;
                }

                *scroll = 0;

                return true;
            },
            _ => { /*  */ },
        }

        false
    }
}