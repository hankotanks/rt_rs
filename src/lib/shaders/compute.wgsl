// NOTE: This won't compile on its own.
// The project automatically inserts intersection logic and 
// additional bind groups based on the `IntrsHandler` that is passed to State.
// This additional code is inserted before `main_cs` and MUST contain
// a function with name and signature: `fn intrs(r: Ray) -> Intrs`

//
// Output Texture

@group(0) @binding(0)
var out: texture_storage_2d<rgba8unorm, write>;

//
// Size Declaration & Binding

struct Size { width: u32, height: u32, }

@group(0) @binding(1)
var<uniform> size: Size;

//
// Config Declaration & Binding

struct Config { 
    t_min: f32, 
    t_max: f32,
    camera_light_source: f32,
    bounces: u32,
    eps: f32,
    ambience: f32,
}

@group(1) @binding(0)
var<uniform> config: Config;

//
// Camera Declaration & Binding

struct Camera { pos: vec3<f32>, at: vec3<f32>, }

@group(2) @binding(0)
var<uniform> camera: Camera;

//
// Geometry

struct Prim {
    a: u32, 
    b: u32, 
    c: u32,
    material: i32,
}

// Array of raw primitives
@group(2) @binding(1)
var<storage, read> primitives: array<Prim>;

// Vertex definition
struct Vertex {
    pos: vec3<f32>,
    normal: vec3<f32>,
}

// Array of vertices
@group(2) @binding(2)
var<storage, read> vertices: array<Vertex>;

struct Light {
    pos: vec3<f32>,
    strength: f32,
}

// Array of lights
@group(2) @binding(3)
var<storage, read> lights: array<Light>;

struct Material {
    color: vec3<f32>,
    albedo: vec3<f32>,
    spec: f32,
}

// Array of materials
@group(2) @binding(4)
var<storage, read> materials: array<Material>;

// Ray declaration
struct Ray { origin: vec3<f32>, dir: vec3<f32>, }

// Intersection type declaration
struct Intrs { s: Prim, t: f32, }

struct Hit { 
    at: vec3<f32>, 
    normal: vec3<f32>, 
    s: Prim,
    t: f32,
}

//
// Raytracer

fn camera_ray(coord: vec2<i32>) -> Ray {
    let dir = normalize(camera.at - camera.pos);

    let up = vec3<f32>(0., 1.0, 0.0);
    let right = cross(dir, up);

    let norm_x = (f32(coord.x) / f32(size.width)) - 0.5;
    let norm_y = (f32(coord.y) / f32(size.height)) - 0.5;

    let i = right * norm_x;
    let j = up * norm_y;

    let pt = i + j + camera.pos + dir;
    
    return Ray(camera.pos, normalize(pt - camera.pos));
}

fn hit(intrs: Intrs, r: Ray) -> Hit {
    let at: vec3<f32> = r.origin + (r.dir * intrs.t);
    // NOTE: As of now, 
    // I have no explanation for why these need to be flipped...
    let b: vec3<f32> = vertices[intrs.s.a].pos;
    let c: vec3<f32> = vertices[intrs.s.b].pos;
    let a: vec3<f32> = vertices[intrs.s.c].pos;

    let v0: vec3<f32> = b - a;
    let v1: vec3<f32> = c - a;
    let v2: vec3<f32> = at - a;

    let d00: f32 = dot(v0, v0);
    let d01: f32 = dot(v0, v1);
    let d11: f32 = dot(v1, v1);
    let d20: f32 = dot(v2, v0);
    let d21: f32 = dot(v2, v1);

    let denom: f32 = d00 * d11 - d01 * d01;

    let v: f32 = (d11 * d20 - d01 * d21) / denom;
    let w: f32 = (d00 * d21 - d01 * d20) / denom;
    let u: f32 = 1.0 - v - w;

    let na: vec3<f32> = vertices[intrs.s.a].normal * v;
    let nb: vec3<f32> = vertices[intrs.s.b].normal * w;
    let nc: vec3<f32> = vertices[intrs.s.c].normal * u;

    let normal = normalize(na + nb + nc);

    return Hit(at, normal, intrs.s, intrs.t);
}

struct LightingPack {
    camera_ray: Ray,
    light: Light,
    hit: Hit,
    material: Material,
}

fn lighting_diffuse(pack: LightingPack) -> f32 {
    let light_dir = normalize(pack.light.pos - pack.hit.at);

    return pack.light.strength * max(0.0, dot(light_dir, pack.hit.normal));
}

fn lighting_spec(pack: LightingPack) -> f32 {
    let light_dir = normalize(pack.light.pos - pack.hit.at);

    let refl: vec3<f32> = reflect(light_dir * -1.0, pack.hit.normal);

    var spec: f32 = dot(-1.0 * refl, pack.camera_ray.dir);
        spec = pow(max(0.0, spec), pack.material.spec) * pack.light.strength;

    return spec;
}

fn intrs_valid(intrs: Intrs) -> bool {
    var valid = intrs.s.material != -1i;
        valid &= intrs.t < config.t_max;
        valid &= intrs.t > config.t_min;

    return valid;
}

fn shadowed(pack: LightingPack) -> bool {
    let light_dir = normalize(pack.light.pos - pack.hit.at);
    let light_dist = length(pack.light.pos - pack.hit.at);

    var shadow_origin: vec3<f32>;
    if(dot(light_dir, pack.hit.normal) < 0.0) {
        shadow_origin = pack.hit.at - pack.hit.normal * 0.001;
    } else {
        shadow_origin = pack.hit.at + pack.hit.normal * 0.001;
    }

    let shadow_ray: Ray = Ray(shadow_origin, light_dir);

    let shadow_intrs = intrs(shadow_ray, pack.hit.s);
    if(intrs_valid(shadow_intrs)) {
        let shadow_hit = hit(shadow_intrs, shadow_ray);

        if(length(shadow_hit.at - shadow_origin) < light_dist) {
            return true;
        }
    }

    return false;
}

fn lighting(camera_ray: Ray) -> vec3<f32> {
    var ray: Ray = camera_ray;

    var color: vec3<f32> = vec3<f32>(0.0);

    for(var i: u32 = 0u; i < config.bounces; i = i + 1u) {
        let intrs: Intrs = intrs(ray, primitives[0]);
        if(!intrs_valid(intrs)) { break; }

        let material: Material = materials[intrs.s.material];

        let hit = hit(intrs, ray);

        var intensity_diffuse: f32 = 0.;
        var intensity_spec: f32 = 0.;

        // Handle the camera light source
        if(config.camera_light_source > 0.0) {
            let pack_light = Light(camera_ray.origin, config.camera_light_source);
            let pack = LightingPack(ray, pack_light, hit, material);

            if(!shadowed(pack)) {
                intensity_diffuse += lighting_diffuse(pack);
                intensity_spec += lighting_spec(pack);
            }
        }

        // Iterate through all other light sources in the scene
        for(var j = 0i; j < i32(arrayLength(&lights)); j = j + 1i) {
            if(lights[j].strength > 0.0) {
                let pack = LightingPack(ray, lights[j], hit, material);

                if(!shadowed(pack)) {
                    intensity_diffuse += lighting_diffuse(pack);
                    intensity_spec += lighting_spec(pack);
                }
            }
        }

        let color_temp = material.color * intensity_diffuse * material.albedo.x + //
            vec3<f32>(1.0) * intensity_spec * material.albedo.y;

        if(i == 0u) {
            color += color_temp;
        } else {
            color += color_temp * material.albedo.z;
        }

        let refl_dir = normalize(reflect(ray.dir, hit.normal));

        var refl_origin: vec3<f32>;
        if(dot(refl_dir, hit.normal) < 0.0) {
            refl_origin = hit.at - hit.normal * 0.001;
        } else {
            refl_origin = hit.at + hit.normal * 0.001;
        }

        ray = Ray(refl_origin, refl_dir);
    }

    return color;
}

// NOTE: The workgroup size is effected by config options, 
// the x & y values are replaced at runtime
@compute @workgroup_size(16, 16, 1)
fn main_cs(@builtin(global_invocation_id) id: vec3<u32>) {
    if(id.x < size.width && id.y < size.height) {
        let coord: vec2<i32> = vec2<i32>(i32(id.x), i32(id.y));

        let color: vec3<f32> = lighting(camera_ray(coord));

        textureStore(out, coord, vec4<f32>(color, 1.0));
    }
}