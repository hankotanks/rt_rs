use std::fmt;

use once_cell::sync::OnceCell;

use crate::{geom, scene};

#[repr(C)]
#[derive(Clone, Copy)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(serde::Serialize)]
#[derive(Debug)]
pub struct Bounds {
    min: [f32; 3],
    #[serde(skip)]
    _p0: u32,
    max: [f32; 3],
    #[serde(skip)]
    _p1: u32,
}

impl<'de> serde::Deserialize<'de> for Bounds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> {
        
        #[derive(serde::Deserialize)]
        struct Intermediate {
            min: Vec<f32>,
            max: Vec<f32>,
        }

        let intermediate = Intermediate::deserialize(deserializer)?;

        let min = match intermediate.min.len() {
            3 => {
                let mut min = [0.; 3];

                min.copy_from_slice(&intermediate.min);
                min
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.min.len(), 
                    &"an array of len 3",
                ));
            }
        };

        let max = match intermediate.max.len() {
            3 => {
                let mut max = [0.; 3];

                max.copy_from_slice(&intermediate.max);
                max
            },
            _ => {
                use serde::de;

                return Err(de::Error::invalid_length(
                    intermediate.max.len(), 
                    &"an array of len 3",
                ));
            }
        };

        Ok(Self {
            min, 
            _p0: 0,
            max, 
            _p1: 0,
        })
    }
}

impl Bounds {
    fn new<P>(prims: P, vertices: &[geom::PrimVertex]) -> Self
        where P: Iterator<Item = geom::Prim> {

        let mut min = [f32::MAX; 3];
        let mut max = [f32::MAX * -1.; 3];

        fn extrema_vertex(
            vertex: [f32; 3], 
            minima: &mut [f32; 3], 
            maxima: &mut [f32; 3],
        ) {
            if vertex[0] < minima[0] { minima[0] = vertex[0]; }
            if vertex[1] < minima[1] { minima[1] = vertex[1]; }
            if vertex[2] < minima[2] { minima[2] = vertex[2]; }

            if vertex[0] > maxima[0] { maxima[0] = vertex[0]; }
            if vertex[1] > maxima[1] { maxima[1] = vertex[1]; }
            if vertex[2] > maxima[2] { maxima[2] = vertex[2]; }
        }

        for geom::Prim { indices: [a, b, c], .. } in prims {
            let a = vertices[a as usize].pos;
            let b = vertices[b as usize].pos;
            let c = vertices[c as usize].pos;

            extrema_vertex(a, &mut min, &mut max);
            extrema_vertex(b, &mut min, &mut max);
            extrema_vertex(c, &mut min, &mut max);
        }

        Self { min, _p0: 0, max, _p1: 0 }
    }

    fn contains(&self, point: [f32; 3]) -> bool {
        point[0] >= self.min[0] &&
        point[0] <= self.max[0] &&
        point[1] >= self.min[1] &&
        point[1] <= self.max[1] &&
        point[2] >= self.min[2] &&
        point[2] <= self.max[2]
    }
}

pub struct Aabb {
    pub fst: OnceCell<Box<Aabb>>,
    pub snd: OnceCell<Box<Aabb>>,
    pub bounds: Bounds,
    pub items: Vec<usize>,
}

impl fmt::Debug for Aabb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.fst.get().is_none() && self.snd.get().is_none() 
            && !self.items.is_empty() {

            return write!(f, "{:?}", self.items);
        }

        let mut out = f.debug_list();

        if let Some(fst) = self.fst.get() {
            out.entry(fst);
        }

        if let Some(snd) = self.snd.get() {
            out.entry(snd);
        }

        out.finish()
    }
}

impl Aabb {
    fn split(
        &mut self, 
        eps: f32,
        prims: &[geom::Prim], 
        vertices: &[geom::PrimVertex],
    ) {
        use geom::V3Ops as _;

        if self.items.len() <= 2 { 
            return;
        }

        let d = self.bounds.max.sub(self.bounds.min);

        let mut fst = Self {
            fst: OnceCell::new(),
            snd: OnceCell::new(),
            bounds: self.bounds,
            items: Vec::new(),
        };

        let mut snd = Self {
            fst: OnceCell::new(),
            snd: OnceCell::new(),
            bounds: self.bounds,
            items: Vec::new(),
        };

        if d[0] >= d[1] && d[0] >= d[2] {
            if d[0] < eps * 0.5 { return; }

            fst.bounds.max[0] = self.bounds.min[0] + d[0] * 0.5;
            snd.bounds.min[0] = fst.bounds.max[0];
        } else if d[1] >= d[2] && d[1] >= d[0] {
            if d[1] < eps * 0.5 { return; }

            fst.bounds.max[1] = self.bounds.min[1] + d[1] * 0.5;
            snd.bounds.min[1] = fst.bounds.max[1];
        } else {
            if d[2] < eps * 0.5 { return; }

            fst.bounds.max[2] = self.bounds.min[2] + d[2] * 0.5;
            snd.bounds.min[2] = fst.bounds.max[2];
        }

        let centroid = |tri: geom::Prim| -> [f32; 3] {
            let [a, b, c] = tri.indices;

            let a = vertices[a as usize].pos;
            let b = vertices[b as usize].pos;
            let c = vertices[c as usize].pos;

            let ab = a.add(b).scale(0.5);
            let bc = b.add(c).scale(0.5);
            let ca = c.add(a).scale(0.5);

            // I'll let the compiler figure out the precision
            (ab.add(bc).add(ca)).scale(1. / 3.)
        };

        for (idx, tri) in self.items.iter().map(|&idx| (idx, prims[idx])) {
            let centroid = centroid(tri);

            if fst.bounds.contains(centroid) {
                fst.items.push(idx);
            } else {
                snd.items.push(idx);
            }
        }

        if fst.items.is_empty() {
            self.bounds = snd.bounds;

            self.split(eps, prims, vertices);
        } else if snd.items.is_empty() {
            self.bounds = fst.bounds;

            self.split(eps, prims, vertices);
        } else {
            self.items.clear();

            fst.bounds = Bounds::new(
                fst.items.iter().map(|&i| prims[i]), 
                vertices
            );

            snd.bounds = Bounds::new(
                snd.items.iter().map(|&i| prims[i]), 
                vertices
            );

            fst.split(eps, prims, vertices);
            snd.split(eps, prims, vertices);

            self.fst.set(Box::new(fst)).unwrap();
            self.snd.set(Box::new(snd)).unwrap();
        }
    }

    pub fn from_scene_unloaded() -> Self {
        Self {
            fst: OnceCell::new(),
            snd: OnceCell::new(),
            bounds: Bounds::new([].into_iter(), &[]),
            items: vec![0],
        }
    }

    pub fn from_scene(
        eps: f32,
        scene: &scene::Scene,
    ) -> Self {
        let scene::Scene::Active { 
            prims, 
            vertices, .. 
        } = scene else {
            return Self::from_scene_unloaded();
        };

        let mut root = Self {
            fst: OnceCell::new(),
            snd: OnceCell::new(),
            bounds: Bounds::new(prims.iter().copied(), vertices),
            items: (0..prims.len()).collect()
        };

        root.split(eps, prims, vertices);
        root
    }
}