use crate::scene;

#[derive(Clone, Copy)]
pub struct BasicIntrs;

impl super::IntrsHandler for BasicIntrs {
    fn vars<'a>(
        _scene: &scene::Scene, 
        device: &wgpu::Device,
    ) -> super::IntrsPack<'a> {
        let layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[]
            }
        );

        let group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[],
            }
        );

        super::IntrsPack {
            vars: Vec::with_capacity(0),
            group,
            layout,
        }
    }

    fn logic() -> &'static str {"\
        fn intrs_empty() -> Intrs {
            return Intrs(primitives[0], config.t_max + 1.0);
        }

        fn intrs_sphere(r: Ray, s: Prim) -> Intrs {        
            let r2 = s.scale * s.scale;
            let l = vertices[s.a].pos - r.origin;
        
            let tca = dot(l, normalize(r.dir));
            let d2 = dot(l, l) - tca * tca;
        
            if(d2 > r2) {
                return intrs_empty();
            }
        
            let thc = sqrt(r2 - d2);
        
            var t = tca - thc;
            var w = tca + thc;
        
            let len = length(r.dir);
        
            if(t < config.t_max && t > config.t_min) {
                t = t / len;
            } else {
                t = -1.0;
            }
        
            if(w < config.t_max && w > config.t_min) {
                w = t / len;
            } else {
                w = -1.0;
            }
        
            if(t > 0.0 && w == -1.0) {
                return Intrs(s, t);
            }
        
            if(w > 0.0 && t == -1.0) {
                return Intrs(s, w);
            }
        
            if(t > 0.0 && w > 0.0) {
                if(t < w) {
                    return Intrs(s, t);
                } else {
                    return Intrs(s, w);
                }
            }
        
            return intrs_empty();
        }

        fn intrs_tri(r: Ray, s: Prim) -> Intrs {
            let e1: vec3<f32> = vertices[s.b].pos - vertices[s.a].pos;
            let e2: vec3<f32> = vertices[s.c].pos - vertices[s.a].pos;

            let p: vec3<f32> = cross(r.dir, e2);
            let t: vec3<f32> = r.origin - vertices[s.a].pos;
            let q: vec3<f32> = cross(t, e1);

            let det = dot(e1, p);

            var u: f32 = 0.0;
            var v: f32 = 0.0;
            if(det > config.eps) {
                u = dot(t, p);
                if(u < 0.0 || u > det) { return intrs_empty(); }

                v = dot(r.dir, q);
                if(v < 0.0 || u + v > det) { return intrs_empty(); }
            } else if(det < -1.0 * config.eps) {
                u = dot(t, p);
                if(u > 0.0 || u < det) { return intrs_empty(); }

                v = dot(r.dir, q);
                if(v > 0.0 || u + v < det) { return intrs_empty(); }
            } else {
                return intrs_empty();
            }

            let w: f32 = dot(e2, q) / det;
            
            if(w > config.t_max || w < config.t_min) {
                return intrs_empty();
            } else {
                return Intrs(s, w);
            }
        }
        
        fn intrs(r: Ray, excl: Prim) -> Intrs {
            var intrs: Intrs = Intrs(primitives[0], config.t_max + 1.0);

            for(var i = 1i; i < i32(arrayLength(&primitives)); i = i + 1i) {
                let prim: Prim = primitives[i];

                var excluded: bool = prim.tag != excl.tag;
                    excluded |= prim.a != excl.a;
                    excluded |= prim.b != excl.b;
                    excluded |= prim.c != excl.c;

                if(excluded) {
                    var intrs_temp: Intrs = intrs_empty();
                    if(prim.tag == 1u) {
                        intrs_temp = intrs_sphere(r, prim);
                    } else if(prim.tag == 2u) {
                        intrs_temp = intrs_tri(r, prim);
                    }

                    var replace: bool = intrs_temp.t < intrs.t;
                        replace &= intrs_temp.t > config.t_min;
                        replace &= intrs_temp.t < config.t_max;

                    if(replace) {
                        intrs = intrs_temp;
                    }
                }
            }
        
            return intrs;
        }
    "}
}