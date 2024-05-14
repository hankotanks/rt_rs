use crate::scene;

pub struct BasicIntrs;

impl super::IntrsHandler for BasicIntrs {
    type Config = ();
    
    fn new(_config: ()) -> anyhow::Result<Self> { Ok(Self) }

    fn vars<'a>(
        &self,
        _scene: &mut scene::Scene, device: &wgpu::Device,
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

    fn logic(&self) -> &'static str {"\
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

                var excluded: bool = false;
                    excluded |= prim.a != excl.a;
                    excluded |= prim.b != excl.b;
                    excluded |= prim.c != excl.c;

                if(excluded) {
                    var intrs_temp: Intrs = intrs_tri(r, prim);

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