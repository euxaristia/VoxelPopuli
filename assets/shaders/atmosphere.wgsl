// POST
@fragment fn fs_main(in:CinematicVsOut)->@location(0)vec4<f32>{
    let size=vec2<i32>(textureDimensions(post_depth));let p=clamp(vec2<i32>(in.uv*vec2<f32>(size)),vec2(0),size-1);let depth=textureLoad(post_depth,p,0);
    let camera=fx.light.camera_pos_exposure.xyz;let endpoint=cinematic_world(in.uv,depth);let dir=normalize(endpoint-camera);let distance=select(min(length(endpoint-camera),512.0),2400.0,depth>=1.0);
    let count=i32(fx.viewport.w);let segment=min(distance,192.0)/f32(count);var radiance=vec3(0.0);var trans=vec3(1.0);
    let cosine=dot(dir,fx.active_light.xyz);let g=0.68;let phase=(1.0-g*g)/(12.56637*pow(max(1.0+g*g-2.0*g*cosine,0.01),1.5));
    let key_color=select(fx.light.moon_color.rgb,fx.light.sun_color.rgb,fx.light.sun_direction_illuminance.w>=fx.light.moon_direction_illuminance.w);
    // Stable world-space samples, no screen-space sun streak and no endpoint-only cave test.
    for(var i=0;i<count;i++){
        let world=camera+dir*((f32(i)+0.5)*segment);let cell=volume_at(world);
        if(cell.r<0.1||cell.r>0.75){continue;}
        let underwater=liquid_at(world);let sky=cell.g;let sun=directional_shadow(world,vec3(0.0));
        let density=fx.light.sky_params.w*exp(-max(world.y-100.0,0.0)*0.008);
        let extinction=select(vec3(density),vec3(0.20,0.08,0.025),underwater);let step_trans=exp(-extinction*segment);
        let outdoor=key_color*fx.active_light.w*sun*sky*phase+analytic_sky(dir)*sky*0.16;
        let water_scatter=fx.light.water_color.rgb*(fx.active_light.w*sky*sun*0.08+fx.light.sky_params.z*0.03);
        let medium=select(outdoor,water_scatter,underwater);
        radiance+=trans*medium*(1.0-step_trans);trans*=step_trans;
    }
    let clouds=cloud_integral(camera,dir,distance,select(24,48,fx.viewport.w>24.0));
    radiance+=trans*clouds.rgb;trans*=clouds.a;
    // RGB transmittance is recovered in the bilateral compositor for underwater wavelengths.
    return vec4(min(radiance,vec3(60000.0)),dot(trans,vec3(0.2126,0.7152,0.0722)));
}
