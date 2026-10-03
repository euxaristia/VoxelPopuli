struct Deferred {inv_view_proj:mat4x4<f32>,camera_pos_exposure:vec4<f32>,sun_direction_illuminance:vec4<f32>,sun_color:vec4<f32>,moon_direction_illuminance:vec4<f32>,moon_color:vec4<f32>,ambient_color_illuminance:vec4<f32>,sky_params:vec4<f32>,zenith_color:vec4<f32>,horizon_color:vec4<f32>,atmosphere:vec4<f32>,horizon_stops:vec4<f32>,block_light_color:vec4<f32>,water_color:vec4<f32>,}
@group(0)@binding(0)var<uniform>u:Deferred;
@group(1)@binding(1)var normal_map:texture_2d<f32>;
@group(1)@binding(4)var depth_map:texture_depth_2d;
struct Out{@builtin(position)pos:vec4<f32>,@location(0)uv:vec2<f32>,}
@vertex fn vs_main(@builtin(vertex_index)i:u32)->Out{let x=f32((i<<1u)&2u);let y=f32(i&2u);var o:Out;o.uv=vec2(x,y);o.pos=vec4(x*2.0-1.0,1.0-y*2.0,0.0,1.0);return o;}
fn world(p:vec2<i32>,size:vec2<i32>,z:f32)->vec3<f32>{let uv=(vec2<f32>(p)+0.5)/vec2<f32>(size);let w=u.inv_view_proj*vec4(uv.x*2.0-1.0,1.0-uv.y*2.0,z,1.0);return w.xyz/w.w;}
@fragment fn fs_main(in:Out)->@location(0)vec4<f32>{
    let size=vec2<i32>(textureDimensions(depth_map));let p=clamp(vec2<i32>(in.uv*vec2<f32>(size)),vec2(0),size-1);let z=textureLoad(depth_map,p,0);
    if(z>=1.0){return vec4(1.0);}
    let origin=world(p,size,z);let n=normalize(textureLoad(normal_map,p,0).xyz);let distance=max(length(origin-u.camera_pos_exposure.xyz),1.0);
    let radius=clamp(f32(size.y)*1.5/distance,2.0,40.0);var occ=0.0;var weight=0.0;
    for(var i=0;i<16;i++){let angle=f32(i)*2.399963;let r=sqrt((f32(i)+0.5)/16.0)*radius;let q=clamp(p+vec2<i32>(vec2(cos(angle),sin(angle))*r),vec2(0),size-1);let sample_z=textureLoad(depth_map,q,0);
        if(sample_z<1.0){let delta=world(q,size,sample_z)-origin;let length2=dot(delta,delta);let range=1.0-smoothstep(0.2,5.0,sqrt(length2));occ+=max(dot(n,delta)/max(sqrt(length2),0.001)-0.08,0.0)*range;weight+=range;}}
    let ao=clamp(1.0-occ/max(weight,1.0)*1.7,0.2,1.0);return vec4(ao,z,n.y*0.5+0.5,1.0);
}
