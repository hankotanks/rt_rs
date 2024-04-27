use std::{fmt, io};

use once_cell::unsync::OnceCell;

use crate::{geom, scene};

#[repr(C)]
#[derive(Clone, Copy)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Debug)]
pub struct Bounds {
    min: [f32; 3],
    _p0: u32,
    max: [f32; 3],
    _p1: u32,
}

impl Bounds {
    fn new<P>(prims: P, vertices: &[geom::PrimVertex]) -> Self
        where P: Iterator<Item = geom::Prim> {

        let mut min = [std::f32::MAX; 3];
        let mut max = [std::f32::MAX * -1.; 3];

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
    ) -> anyhow::Result<()> {
        use geom::v3::V3Ops as _;

        if self.items.len() == 1 { 
            return Ok(()); 
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
            if d[0] < eps * 0.5 { return Ok(()); }

            fst.bounds.max[0] = self.bounds.min[0] + d[0] * 0.5;
            snd.bounds.min[0] = fst.bounds.max[0];
        } else if d[1] >= d[2] && d[1] >= d[0] {
            if d[1] < eps * 0.5 { return Ok(()); }

            fst.bounds.max[1] = self.bounds.min[1] + d[1] * 0.5;
            snd.bounds.min[1] = fst.bounds.max[1];
        } else {
            if d[2] < eps * 0.5 { return Ok(()); }

            fst.bounds.max[2] = self.bounds.min[2] + d[2] * 0.5;
            snd.bounds.min[2] = fst.bounds.max[2];
        }

        let centroid = |tri: geom::Prim| -> [f32; 3] {
            let [a, b, c] = tri.indices;

            let a = vertices[a as usize].pos;
            let b = vertices[b as usize].pos;
            let c = vertices[c as usize].pos;

            // I'll let the compiler figure out the precision
            a.add(b).add(c).scale(1. / 3.)
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

            self.split(eps, prims, vertices)?;
        } else if snd.items.is_empty() {
            self.bounds = fst.bounds;

            self.split(eps, prims, vertices)?;
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

            fst.split(eps, prims, vertices)?;
            snd.split(eps, prims, vertices)?;

            self.fst.set(Box::new(fst))
                .map_err(|_| io::Error::from(io::ErrorKind::AlreadyExists))?;

            self.snd.set(Box::new(snd))
                .map_err(|_| io::Error::from(io::ErrorKind::AlreadyExists))?;
        }

        Ok(())
    }

    pub fn from_scene(
        eps: f32,
        scene: &scene::Scene,
    ) -> anyhow::Result<Self> {
        let scene::Scene::Active { prims, vertices, .. } = scene else {
            anyhow::bail!("\
                Unable to construct an axis-aligned bounding box \
                from an unloaded scene\
            ");
        };

        let mut root = Self {
            fst: OnceCell::new(),
            snd: OnceCell::new(),
            bounds: Bounds::new(prims.iter().copied(), vertices),
            items: (0..prims.len()).collect()
        };

        root.split(eps, prims, vertices)?;

        Ok(root)
    }
}