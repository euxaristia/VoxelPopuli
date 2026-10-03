struct CinematicLighting {
    inv_view_proj: mat4x4<f32>,
    camera_pos_exposure: vec4<f32>,
    sun_direction_illuminance: vec4<f32>,
    sun_color: vec4<f32>,
    moon_direction_illuminance: vec4<f32>,
    moon_color: vec4<f32>,
    ambient_color_illuminance: vec4<f32>,
    sky_params: vec4<f32>,
    zenith_color: vec4<f32>,
    horizon_color: vec4<f32>,
    atmosphere: vec4<f32>,
    horizon_stops: vec4<f32>,
    block_light_color: vec4<f32>,
    water_color: vec4<f32>,
}
struct CinematicEffects {
    view_proj: mat4x4<f32>,
    previous_view_proj: mat4x4<f32>,
    shadows: array<mat4x4<f32>,4>,
    splits: vec4<f32>,
    viewport: vec4<f32>,
    time: vec4<f32>,
    volume_origin: vec4<f32>,
    active_light: vec4<f32>,
    light: CinematicLighting,
}
@group(2) @binding(0) var<uniform> fx: CinematicEffects;
@group(2) @binding(1) var shadow_map: texture_depth_2d_array;
@group(2) @binding(2) var shadow_sampler: sampler_comparison;
@group(2) @binding(3) var block_volume: texture_3d<f32>;
@group(2) @binding(4) var opaque_scene: texture_2d<f32>;
@group(2) @binding(5) var opaque_depth: texture_depth_2d;
@group(2) @binding(6) var contact_ao: texture_2d<f32>;
@group(2) @binding(7) var water_depth_map: texture_depth_2d;

fn cinematic_world(uv:vec2<f32>,depth:f32)->vec3<f32>{let p=fx.light.inv_view_proj*vec4(uv.x*2.0-1.0,1.0-uv.y*2.0,depth,1.0);return p.xyz/p.w;}
fn volume_at(world:vec3<f32>)->vec4<f32>{let p=vec3<i32>(floor(world-fx.volume_origin.xyz));let size=vec3<i32>(textureDimensions(block_volume));if(any(p<vec3(0))||any(p>=size)){return vec4(0.0);}return textureLoad(block_volume,p,0);}
fn liquid_at(world:vec3<f32>)->bool{let cell=volume_at(world);return cell.r>0.1&&cell.b>0.5&&fract(world.y)<cell.a;}
fn shadow_layer(world:vec3<f32>,normal:vec3<f32>,layer:i32)->f32{
    let bias=normal*(0.015+0.045*(1.0-abs(dot(normal,fx.active_light.xyz))));
    let p=fx.shadows[layer]*vec4(world+bias,1.0);let q=p.xyz/p.w;let uv=vec2(q.x*0.5+0.5,0.5-q.y*0.5);
    if(any(uv<vec2(0.001))||any(uv>vec2(0.999))||q.z<=0.0||q.z>=1.0){return 1.0;}
    var visibility=0.0;
    for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){visibility+=textureSampleCompareLevel(shadow_map,shadow_sampler,uv+vec2(f32(x),f32(y))*fx.viewport.z*1.4,layer,q.z-0.00012);}}
    return visibility/9.0;
}
fn directional_shadow(world:vec3<f32>,normal:vec3<f32>)->f32{
    let distance=length(world-fx.light.camera_pos_exposure.xyz);var layer=0;
    for(var i=0;i<3;i++){if(distance>fx.splits[i]){layer=i+1;}}
    let a=shadow_layer(world,normal,layer);
    if(layer<3){let start=fx.splits[layer]*0.9;let blend=smoothstep(start,fx.splits[layer],distance);if(blend>0.0){return mix(a,shadow_layer(world,normal,layer+1),blend);}}
    return a;
}
fn noise_hash(p:vec3<f32>)->f32{return fract(sin(dot(p,vec3(127.1,311.7,74.7)))*43758.5453);}
fn volume_noise(p:vec3<f32>)->f32{
    let i=floor(p);let f=fract(p);let t=f*f*(3.0-2.0*f);
    let a=mix(noise_hash(i),noise_hash(i+vec3(1.0,0.0,0.0)),t.x);let b=mix(noise_hash(i+vec3(0.0,1.0,0.0)),noise_hash(i+vec3(1.0,1.0,0.0)),t.x);
    let c=mix(noise_hash(i+vec3(0.0,0.0,1.0)),noise_hash(i+vec3(1.0,0.0,1.0)),t.x);let d=mix(noise_hash(i+vec3(0.0,1.0,1.0)),noise_hash(i+vec3(1.0)),t.x);
    return mix(mix(a,b,t.y),mix(c,d,t.y),t.z);
}
fn cloud_density(world:vec3<f32>)->f32{
    let height=(world.y-164.0)/34.0;if(height<=0.0||height>=1.0){return 0.0;}
    let p=(world+vec3(fx.time.x*0.8,0.0,fx.time.x*0.24))*0.012;
    let shape=volume_noise(p)+0.5*volume_noise(p*2.03)+0.25*volume_noise(p*4.09);
    let edge=smoothstep(0.0,0.18,height)*(1.0-smoothstep(0.65,1.0,height));
    return clamp((shape-0.85)*2.5,0.0,1.0)*edge;
}
fn analytic_sky(dir:vec3<f32>)->vec3<f32>{
    let t=pow(clamp(dir.y*1.4,0.0,1.0),1.0/max(fx.light.atmosphere.x,0.2));
    var color=mix(fx.light.horizon_color.rgb,fx.light.zenith_color.rgb,t);
    let key=max(fx.light.sun_direction_illuminance.w+fx.light.moon_direction_illuminance.w,0.0001);
    color+=fx.light.sun_color.rgb*fx.light.atmosphere.y*pow(max(dot(dir,fx.light.sun_direction_illuminance.xyz),0.0),1.0+fx.light.atmosphere.w)*fx.light.sun_direction_illuminance.w/key;
    color+=fx.light.moon_color.rgb*fx.light.atmosphere.z*pow(max(dot(dir,fx.light.moon_direction_illuminance.xyz),0.0),12.0)*fx.light.moon_direction_illuminance.w/key;
    return color*key*0.318309886;
}
fn cloud_integral(origin:vec3<f32>,dir:vec3<f32>,max_distance:f32,steps:i32)->vec4<f32>{
    if(abs(dir.y)<0.0001){return vec4(0.0,0.0,0.0,1.0);}
    let a=(164.0-origin.y)/dir.y;let b=(198.0-origin.y)/dir.y;
    let begin=max(min(a,b),0.0);let end=min(min(max(a,b),max_distance),2400.0);
    if(end<=begin){return vec4(0.0,0.0,0.0,1.0);}
    let step=(end-begin)/f32(steps);var trans=1.0;var radiance=vec3(0.0);
    let light_dir=fx.active_light.xyz;let light_color=select(fx.light.moon_color.rgb,fx.light.sun_color.rgb,fx.light.sun_direction_illuminance.w>=fx.light.moon_direction_illuminance.w);
    let key=fx.active_light.w;let phase=0.25+0.75*pow(max(dot(dir,light_dir),0.0),8.0);
    for(var i=0;i<steps;i++){
        let p=origin+dir*(begin+(f32(i)+0.5)*step);let density=cloud_density(p);
        if(density>0.001){var optical=0.0;for(var j=1;j<=4;j++){optical+=cloud_density(p+light_dir*f32(j)*6.0)*6.0;}
            let lighting=light_color*key*(exp(-optical*0.12)*phase+0.08)*0.318309886+fx.light.horizon_color.rgb*key*0.04;
            let opacity=1.0-exp(-density*step*0.12);radiance+=trans*lighting*opacity;trans*=1.0-opacity;
            if(trans<0.01){break;}
        }
    }
    return vec4(radiance,trans);
}
fn sky_environment(origin:vec3<f32>,dir:vec3<f32>)->vec3<f32>{let clouds=cloud_integral(origin,dir,2400.0,20);return analytic_sky(dir)*clouds.a+clouds.rgb;}
fn scene_color(uv:vec2<f32>,roughness:f32)->vec3<f32>{
    let size=vec2<i32>(textureDimensions(opaque_scene));let p=vec2<i32>(uv*vec2<f32>(size));let r=i32(roughness*5.0);var color=textureLoad(opaque_scene,clamp(p,vec2(0),size-1),0).rgb*0.4;
    color+=textureLoad(opaque_scene,clamp(p+vec2(r,0),vec2(0),size-1),0).rgb*0.15;color+=textureLoad(opaque_scene,clamp(p-vec2(r,0),vec2(0),size-1),0).rgb*0.15;
    color+=textureLoad(opaque_scene,clamp(p+vec2(0,r),vec2(0),size-1),0).rgb*0.15;color+=textureLoad(opaque_scene,clamp(p-vec2(0,r),vec2(0),size-1),0).rgb*0.15;return color;
}
fn scene_reflection(origin:vec3<f32>,dir:vec3<f32>,roughness:f32)->vec4<f32>{
    var previous=0.12;var stride=0.18;let count=i32(fx.viewport.w);let size=vec2<i32>(textureDimensions(opaque_depth));
    for(var i=0;i<count;i++){
        let distance=previous+stride;let ray=origin+dir*distance;let clip=fx.view_proj*vec4(ray,1.0);
        if(clip.w<=0.0){break;}let ndc=clip.xyz/clip.w;let uv=vec2(ndc.x*0.5+0.5,0.5-ndc.y*0.5);
        if(any(uv<vec2(0.0))||any(uv>vec2(1.0))||ndc.z>=1.0){break;}
        let pixel=clamp(vec2<i32>(uv*vec2<f32>(size)),vec2(0),size-1);let depth=textureLoad(opaque_depth,pixel,0);
        if(depth<1.0){let surface=cinematic_world(uv,depth);let delta=length(ray-fx.light.camera_pos_exposure.xyz)-length(surface-fx.light.camera_pos_exposure.xyz);
            if(delta>0.0&&delta<max(0.22,stride*1.6)){
                var lo=previous;var hi=distance;var refined=uv;
                for(var j=0;j<5;j++){let mid=(lo+hi)*0.5;let q=fx.view_proj*vec4(origin+dir*mid,1.0);let uv2=vec2(q.x/q.w*0.5+0.5,0.5-q.y/q.w*0.5);let p2=clamp(vec2<i32>(uv2*vec2<f32>(size)),vec2(0),size-1);let z=textureLoad(opaque_depth,p2,0);if(q.z/q.w>z){hi=mid;}else{lo=mid;}refined=uv2;}
                let edge=min(min(refined.x,refined.y),min(1.0-refined.x,1.0-refined.y));let confidence=smoothstep(0.0,0.1,edge)*(1.0-smoothstep(35.0,80.0,distance))*(1.0-roughness*0.7);
                return vec4(scene_color(refined,roughness),confidence);
            }
        }
        previous=distance;stride*=1.11;if(previous>80.0){break;}
    }
    return vec4(0.0);
}
struct CinematicVsOut {@builtin(position)pos:vec4<f32>,@location(0)uv:vec2<f32>,}
fn fullscreen_vertex(index:u32)->CinematicVsOut{let x=f32((index<<1u)&2u);let y=f32(index&2u);var out:CinematicVsOut;out.uv=vec2(x,y);out.pos=vec4(x*2.0-1.0,1.0-y*2.0,0.0,1.0);return out;}
