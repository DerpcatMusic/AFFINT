// Original AFFINT display policy. Source 0: straight sRGB RGBA8 reference.
// Source 1: canonical linear-premultiplied working frames / diagnostic input.
struct Parameters { page: vec4<f32>, mode: u32, checker_scale: f32, source_offset: vec2<i32> }
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Parameters;

fn decode(v: vec3<f32>) -> vec3<f32> {
    return select(pow((v + vec3(0.055)) / 1.055, vec3(2.4)), v / 12.92, v <= vec3(0.04045));
}
fn encode(v: vec3<f32>) -> vec3<f32> {
    let c = clamp(v, vec3(0.0), vec3(1.0));
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - vec3(0.055), 12.92 * c, c <= vec3(0.0031308));
}
@vertex fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(positions[index], 0.0, 1.0);
}
@fragment fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy) + params.source_offset;
    let dimensions = vec2<i32>(textureDimensions(source));
    // Host coverage validation permits missing samples only outside conservative
    // page AA support, where the evaluated document is canonical transparent.
    var color = vec4(0.0);
    if all(pixel >= vec2(0)) && all(pixel < dimensions) {
        color = textureLoad(source, pixel, 0);
    }
    let alpha = clamp(color.a, 0.0, 1.0);
    // Working frames already canonicalize zero-alpha RGB. An arithmetic alpha
    // comparison may flush a subnormal alpha and discard visible normal HDR RGB.
    var premul = color.rgb;
    if params.mode == 0u { premul = decode(color.rgb) * alpha; }
    let in_page = all(position.xy >= params.page.xy) && all(position.xy < params.page.zw);
    let cell = vec2<i32>(floor(position.xy / max(params.checker_scale, 1.0)));
    let checker = select(0.90, 0.78, ((cell.x + cell.y) & 1) == 0);
    let background = decode(vec3(select(0.115, checker, in_page)));
    // Composite BEFORE nonlinear conversion; the display texture is opaque.
    return vec4(encode(premul + background * (1.0 - alpha)), 1.0);
}
