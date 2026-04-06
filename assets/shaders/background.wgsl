#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> time: f32;

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pos = p;
    for (var i = 0; i < 5; i++) {
        v += a * noise(pos);
        pos = pos * 2.0 + vec2<f32>(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let t = time * 0.05;

    // Warped domain — uv gets distorted by noise itself
    let warp1 = vec2<f32>(fbm(uv * 4.0 + vec2<f32>(t, t * 0.6)), fbm(uv * 4.0 + vec2<f32>(t * 0.8, -t)));
    let warp2 = vec2<f32>(fbm(uv * 4.0 + warp1 * 2.0 + vec2<f32>(1.7, 9.2)), fbm(uv * 4.0 + warp1 * 2.0 + vec2<f32>(8.3, 2.8)));
    let n = fbm(uv * 4.0 + warp2 * 1.5);

    // Color bands that shift with the warping
    let band = sin(n * 12.0 + t * 2.0) * 0.5 + 0.5;

    // Palette: dark teal, deep purple, warm black
    let col1 = vec3<f32>(0.01, 0.04, 0.06); // dark teal
    let col2 = vec3<f32>(0.06, 0.01, 0.08); // deep purple
    let col3 = vec3<f32>(0.04, 0.02, 0.02); // warm dark

    var col = mix(col1, col2, n);
    col = mix(col, col3, band * 0.5);

    // Bright contour lines along warp boundaries
    let edge = abs(fract(n * 6.0) - 0.5);
    let line = smoothstep(0.0, 0.05, edge);
    let glow = (1.0 - line) * 0.12;
    col += vec3<f32>(glow * 0.2, glow * 0.5, glow * 0.9);

    return vec4<f32>(col, 1.0);
}
