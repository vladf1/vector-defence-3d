//! Routes the math functions Rust would otherwise link from `compiler_builtins`' libm (in both
//! f64 and f32 flavors, about 20 KB of Wasm) to the browser's `Math`, which engines implement
//! natively. The symbols below take precedence over libm's weak definitions at link time.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Math, js_name = sin)]
    fn math_sin(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = cos)]
    fn math_cos(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = tan)]
    fn math_tan(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = atan)]
    fn math_atan(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = atan2)]
    fn math_atan2(y: f64, x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = exp)]
    fn math_exp(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = log)]
    fn math_log(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = log10)]
    fn math_log10(x: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = pow)]
    fn math_pow(x: f64, y: f64) -> f64;
    #[wasm_bindgen(js_namespace = Math, js_name = hypot)]
    fn math_hypot(x: f64, y: f64) -> f64;
}

#[unsafe(no_mangle)]
pub extern "C" fn sin(x: f64) -> f64 {
    math_sin(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn cos(x: f64) -> f64 {
    math_cos(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn tan(x: f64) -> f64 {
    math_tan(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn atan(x: f64) -> f64 {
    math_atan(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn atan2(y: f64, x: f64) -> f64 {
    math_atan2(y, x)
}
#[unsafe(no_mangle)]
pub extern "C" fn exp(x: f64) -> f64 {
    math_exp(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn log(x: f64) -> f64 {
    math_log(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn log10(x: f64) -> f64 {
    math_log10(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn pow(x: f64, y: f64) -> f64 {
    math_pow(x, y)
}
#[unsafe(no_mangle)]
pub extern "C" fn hypot(x: f64, y: f64) -> f64 {
    math_hypot(x, y)
}
#[unsafe(no_mangle)]
pub extern "C" fn sinf(x: f32) -> f32 {
    math_sin(f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn cosf(x: f32) -> f32 {
    math_cos(f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn tanf(x: f32) -> f32 {
    math_tan(f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn atanf(x: f32) -> f32 {
    math_atan(f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn atan2f(y: f32, x: f32) -> f32 {
    math_atan2(f64::from(y), f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn expf(x: f32) -> f32 {
    math_exp(f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn logf(x: f32) -> f32 {
    math_log(f64::from(x)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn powf(x: f32, y: f32) -> f32 {
    math_pow(f64::from(x), f64::from(y)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn hypotf(x: f32, y: f32) -> f32 {
    math_hypot(f64::from(x), f64::from(y)) as f32
}
