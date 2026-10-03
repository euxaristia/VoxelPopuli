use crate::vibrant::{VibrantPack, frame, keyframe::Color};
use glam::{Mat4, Vec3};
use wgpu::naga::{BinaryOperator, Expression, Function, Handle, Literal, MathFunction, Statement};

fn scalar(function: &Function, expression: Handle<Expression>, arguments: &[f32]) -> f32 {
    let evaluate = |handle| scalar(function, handle, arguments);
    match function.expressions[expression] {
        Expression::Literal(Literal::F32(value)) => value,
        Expression::Literal(Literal::AbstractFloat(value)) => value as f32,
        Expression::FunctionArgument(index) => arguments[index as usize],
        Expression::Binary { op, left, right } => match op {
            BinaryOperator::Add => evaluate(left) + evaluate(right),
            BinaryOperator::Subtract => evaluate(left) - evaluate(right),
            BinaryOperator::Multiply => evaluate(left) * evaluate(right),
            BinaryOperator::Divide => evaluate(left) / evaluate(right),
            _ => panic!("unsupported scalar operation: {op:?}"),
        },
        Expression::Math { fun, arg, arg1, .. } => match fun {
            MathFunction::Max => evaluate(arg).max(evaluate(arg1.unwrap())),
            MathFunction::Min => evaluate(arg).min(evaluate(arg1.unwrap())),
            _ => panic!("unsupported scalar function: {fun:?}"),
        },
        ref expression => panic!("unsupported scalar expression: {expression:?}"),
    }
}

fn emission_scale(source: &str, illuminance: f32, exposure: f32) -> f32 {
    let module = wgpu::naga::front::wgsl::parse_str(source).unwrap();
    let (handle, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name.as_deref() == Some("emissive_radiance"))
        .expect("lighting shader must expose the emission calculation");
    let fragment = module
        .entry_points
        .iter()
        .find(|entry| entry.name == "fs_main")
        .unwrap();
    assert!(
        fragment.function.body.iter().any(|statement| {
            matches!(statement, Statement::Call { function, .. } if *function == handle)
        }),
        "the fragment must use the tested emission calculation"
    );
    let result = function
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::Return { value } => *value,
            _ => None,
        })
        .unwrap();
    scalar(function, result, &[illuminance, exposure])
}

fn shaders() -> [String; 2] {
    [
        super::cinematic::compose(include_str!("../../assets/shaders/cinematic_lighting.wgsl")),
        include_str!("../../assets/shaders/deferred_lighting.wgsl").to_owned(),
    ]
}

fn frame_at(day_fraction: f32) -> super::DeferredUniforms {
    frame::build_uniforms(
        &VibrantPack::default(),
        &frame::FrameInput {
            day_fraction,
            camera_pos: Vec3::new(0.0, 130.0, 0.0),
            view_proj: Mat4::IDENTITY,
        },
    )
}

#[test]
fn nighttime_emission_stays_bounded_in_display_space() {
    for source in shaders() {
        for time in [0.0, 0.2, 0.25, 0.4, 0.5, 0.6, 0.75, 0.8] {
            let light = frame_at(time);
            let lux = light.sun_direction_illuminance[3] + light.moon_direction_illuminance[3];
            let exposure = light.camera_pos_exposure[3];
            let exposed = emission_scale(&source, lux, exposure) * exposure;
            assert!(
                exposed.is_finite() && exposed <= 2.01,
                "time={time}: full emission has exposed radiance {exposed}"
            );
            assert!(exposed >= 0.2, "emission must remain visible in daylight");
        }
    }
}

#[test]
fn daytime_cinematic_emission_keeps_its_existing_strength() {
    let [source, _] = shaders();
    let light = frame_at(0.0);
    let lux = light.sun_direction_illuminance[3] + light.moon_direction_illuminance[3];
    let actual = emission_scale(&source, lux, light.camera_pos_exposure[3]);
    assert_eq!(actual, (lux * 0.12).max(16.0));
}

#[test]
fn actual_torch_head_does_not_clip_every_colour_channel_white() {
    let atlas = crate::atlas::generate_atlas_data();
    let (tile_x, tile_y) = crate::item::atlas_uv(crate::block::BlockType::Torch);
    let offset = (((tile_y as usize * 16 + 3) * 256) + tile_x as usize * 16 + 7) * 4;
    let linear = Color::rgb(
        atlas[offset] as f32 / 255.0,
        atlas[offset + 1] as f32 / 255.0,
        atlas[offset + 2] as f32 / 255.0,
    )
    .to_linear();
    let light = frame_at(0.5);
    let luminance = linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
    let tint = linear
        .map(|channel| channel * (1.0 - light.sky_params[1]) + luminance * light.sky_params[1]);
    for source in shaders() {
        let energy = emission_scale(
            &source,
            light.sun_direction_illuminance[3] + light.moon_direction_illuminance[3],
            light.camera_pos_exposure[3],
        ) * light.camera_pos_exposure[3];
        let exposed = tint.map(|channel| channel * energy);
        assert!(
            exposed[2] < 1.0,
            "torch blue channel must not clip: {exposed:?}"
        );
        assert!(
            exposed[0] > exposed[2] * 3.0,
            "torch must retain its warm emission"
        );
    }
}
