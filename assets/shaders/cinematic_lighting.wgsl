@group(0) @binding(0) var<uniform> light:CinematicLighting;
@group(1) @binding(0) var albedo_map:texture_2d<f32>;
@group(1) @binding(1) var normal_map:texture_2d<f32>;
@group(1) @binding(2) var material_map:texture_2d<f32>;
@group(1) @binding(3) var baked_map:texture_2d<f32>;
@group(1) @binding(4) var world_depth:texture_depth_2d;
@vertex fn vs_main(@builtin(vertex_index)i:u32)->CinematicVsOut{return fullscreen_vertex(i);}
fn unit_or(value:vec3<f32>,fallback:vec3<f32>)->vec3<f32> {
    let squared = dot(value, value);
    if (squared > 0.000000000001 && squared < 100000000000000000000.0) {
        return value * inverseSqrt(squared);
    }
    return fallback;
}

fn emissive_radiance(illuminance:f32,exposure:f32)->f32 {
    // Preserve daylight visibility without exposure turning night emitters white.
    return min(max(16.0, illuminance * 0.12), 2.0 / max(exposure, 0.000001));
}
fn brdf(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>,albedo:vec3<f32>,metal:f32,rough:f32,energy:vec3<f32>)->vec3<f32>{
    let nl=max(dot(n,l),0.0);
    if (nl <= 0.0 || max(energy.r,max(energy.g,energy.b)) <= 0.0) { return vec3(0.0); }
    let nv=max(dot(n,v),0.001);let h=unit_or(v+l,n);let nh=max(dot(n,h),0.0);let vh=max(dot(v,h),0.0);
    let a=rough*rough;let a2=a*a;let denominator=nh*nh*(a2-1.0)+1.0;let distribution=a2/max(3.14159265*denominator*denominator,0.000001);
    let vis=0.5/max(nl*sqrt(nv*nv*(1.0-a2)+a2)+nv*sqrt(nl*nl*(1.0-a2)+a2),0.000001);
    let f0=mix(vec3(0.04),albedo,metal);let fresnel=f0+(1.0-f0)*pow(1.0-vh,5.0);
    return ((1.0-fresnel)*(1.0-metal)*albedo*0.318309886+distribution*vis*fresnel)*energy*nl;
}
@fragment fn fs_main(in:CinematicVsOut)->@location(0)vec4<f32>{
    let p=vec2<i32>(in.pos.xy);let depth=textureLoad(world_depth,p,0);let world=cinematic_world(in.uv,depth);let v=unit_or(light.camera_pos_exposure.xyz-world,vec3(0.0,0.0,1.0));
    if(depth>=1.0){return vec4(analytic_sky(-v),1.0);}
    let albedo=textureLoad(albedo_map,p,0).rgb;let n=unit_or(textureLoad(normal_map,p,0).xyz,vec3(0.0,1.0,0.0));let material=textureLoad(material_map,p,0);let baked=textureLoad(baked_map,p,0);
    let rough=clamp(material.b,0.065,1.0);let metal=material.r;let visibility=directional_shadow(world,n)*baked.r;
    var sun_shadow=baked.r;var moon_shadow=baked.r;if(light.sun_direction_illuminance.w>=light.moon_direction_illuminance.w){sun_shadow=visibility;}else{moon_shadow=visibility;}
    var color=brdf(n,v,light.sun_direction_illuminance.xyz,albedo,metal,rough,light.sun_color.rgb*light.sun_direction_illuminance.w)*sun_shadow;
    color+=brdf(n,v,light.moon_direction_illuminance.xyz,albedo,metal,rough,light.moon_color.rgb*light.moon_direction_illuminance.w)*moon_shadow;
    let ao_size=vec2<i32>(textureDimensions(contact_ao));let ap=clamp(p/2,vec2(0),ao_size-1);let contact=textureLoad(contact_ao,ap,0).r;
    // Add contact AO only where baked voxel AO has not already darkened the corner.
    let ao=baked.b*mix(1.0,contact,0.45*baked.b);
    let sky_lux=light.sun_direction_illuminance.w+light.moon_direction_illuminance.w;
    let sky_sample=mix(light.horizon_color.rgb,light.zenith_color.rgb,n.y*0.5+0.5);let sky_color=sky_sample/max(max(sky_sample.r,sky_sample.g),max(sky_sample.b,0.01));
    color+=albedo*(1.0-metal)*sky_color*sky_lux*light.sky_params.x*baked.r*ao*0.318309886;
    let reflected=reflect(-v,n);let fresnel=mix(vec3(0.04),albedo,metal)+(1.0-mix(vec3(0.04),albedo,metal))*pow(1.0-max(dot(n,v),0.0),5.0);
    color+=fresnel*mix(sky_environment(world,reflected),sky_color*sky_lux*0.318309886,rough)*baked.r*ao*light.sky_params.x;
    color+=albedo*light.block_light_color.rgb*baked.g*light.sky_params.z*ao*0.318309886;
    color+=albedo*light.ambient_color_illuminance.rgb*light.ambient_color_illuminance.w*ao*0.318309886;
    let emission=mix(albedo,vec3(dot(albedo,vec3(0.2126,0.7152,0.0722))),light.sky_params.y);
    color+=emission*material.g*emissive_radiance(sky_lux,light.camera_pos_exposure.w);
    let back=pow(max(dot(v,-fx.active_light.xyz),0.0),2.0);color+=albedo*material.a*back*visibility*fx.active_light.w*0.04;
    // Only actual liquid cells enable caustics. Dry caves below sea level never do.
    let above=volume_at(world+n*0.2+vec3(0.0,0.25,0.0));
    if(above.b>0.5&&above.r>0.1){let phase=world.xz*light.moon_color.w;let wave=(sin(phase.x*2.2+fx.time.x)+sin(dot(phase,vec2(0.174,0.985))*2.2-fx.time.x*1.1)+sin(dot(phase,vec2(-0.94,0.342))*2.2+fx.time.x*1.2))/3.0;
        let caustic=pow(clamp(wave*0.5+0.5,0.0,1.0),max(light.block_light_color.w,1.0));color+=albedo*fx.active_light.w*visibility*caustic*max(n.y,0.0)*0.12;}
    return vec4(min(color,vec3(60000.0)),1.0);
}
