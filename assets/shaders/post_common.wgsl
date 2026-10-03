@group(0) @binding(0) var<uniform> frame_light:CinematicLighting;
@group(1) @binding(0) var post_scene:texture_2d<f32>;
@group(1) @binding(1) var post_depth:texture_depth_2d;
@group(1) @binding(2) var post_normal:texture_2d<f32>;
@group(1) @binding(3) var post_history:texture_2d<f32>;
@group(1) @binding(4) var post_history_depth:texture_depth_2d;
@group(1) @binding(5) var post_history_normal:texture_2d<f32>;
@group(1) @binding(6) var post_extra:texture_2d<f32>;
@group(1) @binding(7) var post_material:texture_2d<f32>;
fn load_color(tex:texture_2d<f32>,uv:vec2<f32>)->vec4<f32>{let size=vec2<i32>(textureDimensions(tex));return textureLoad(tex,clamp(vec2<i32>(uv*vec2<f32>(size)),vec2(0),size-1),0);}
fn bilinear_color(tex:texture_2d<f32>,uv:vec2<f32>)->vec4<f32>{let size=vec2<i32>(textureDimensions(tex));let p=uv*vec2<f32>(size)-0.5;let base=vec2<i32>(floor(p));let f=fract(p);let a=textureLoad(tex,clamp(base,vec2(0),size-1),0);let b=textureLoad(tex,clamp(base+vec2(1,0),vec2(0),size-1),0);let c=textureLoad(tex,clamp(base+vec2(0,1),vec2(0),size-1),0);let d=textureLoad(tex,clamp(base+vec2(1),vec2(0),size-1),0);return mix(mix(a,b,f.x),mix(c,d,f.x),f.y);}
@vertex fn vs_main(@builtin(vertex_index)i:u32)->CinematicVsOut{return fullscreen_vertex(i);}
