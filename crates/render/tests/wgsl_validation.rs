//! Parses and validates every generated WGSL module with naga, for both profiles and with a
//! shader salt, so WGSL mistakes surface before a browser run.
use naga::valid::{Capabilities, ValidationFlags, Validator};
use vd_core::profile::{GameMode, GameProfile};
use vd_render::scene_constants::create_scene_constants;
use vd_render::shaders::{ShaderSources, create_shader_sources, salt_shader};

fn modules(sources: &ShaderSources) -> [(&'static str, &str); 6] {
    [
        ("neon", &sources.neon),
        ("ground", &sources.ground),
        ("road", &sources.road),
        ("effect", &sources.effect),
        ("sprite", &sources.sprite),
        ("post", &sources.post),
    ]
}

fn validate(label: &str, code: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(code)
        .unwrap_or_else(|error| panic!("{label}: WGSL parse error:\n{}", error.emit_to_string(code)));
    Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{label}: WGSL validation error:\n{}", error.emit_to_string(code)));
    module
}

#[test]
fn every_generated_module_is_valid_wgsl() {
    for mode in [GameMode::Desktop, GameMode::Mobile] {
        let profile = GameProfile::for_mode(mode);
        let sources = create_shader_sources(&create_scene_constants(
            profile.field_width,
            profile.field_height,
            profile.road_width,
        ));
        for (label, code) in modules(&sources) {
            let module = validate(label, code);
            let entry_points: Vec<&str> = module.entry_points.iter().map(|entry| entry.name.as_str()).collect();
            assert!(entry_points.contains(&"vertexMain") && entry_points.contains(&"fragmentMain"), "{label}");
            validate(label, &salt_shader(code, 7));
        }
    }
}

#[test]
fn only_neon_and_post_fragments_read_resources() {
    // Safari rule: fragment code that reads a buffer or texture compiles far slower, so the
    // ground/road/effect/sprite fragments must stay math-only (lights arrive as varyings).
    let profile = GameProfile::for_mode(GameMode::Desktop);
    let sources =
        create_shader_sources(&create_scene_constants(profile.field_width, profile.field_height, profile.road_width));
    for (label, code) in modules(&sources) {
        let module = validate(label, code);
        let fragment = module.entry_points.iter().find(|entry| entry.name == "fragmentMain").unwrap();
        let expected = matches!(label, "neon" | "post");
        assert_eq!(reads_resources(&module, &fragment.function, 0), expected, "{label}");
    }
}

/// Whether `function` (or anything it calls) reads a uniform buffer or texture/sampler global.
fn reads_resources(module: &naga::Module, function: &naga::Function, depth: u32) -> bool {
    assert!(depth < 16, "unexpected call depth");
    function.expressions.iter().any(|(_, expression)| match *expression {
        naga::Expression::GlobalVariable(handle) => {
            module.global_variables[handle].space != naga::AddressSpace::Private
        }
        naga::Expression::CallResult(callee) => reads_resources(module, &module.functions[callee], depth + 1),
        _ => false,
    })
}
