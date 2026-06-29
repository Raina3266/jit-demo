use jit_demo::jit::JIT;

const SOURCE: &str = r#"
fn add(x, y) {
    return x + y;
}

fn main() {
    return add(2, 3);
}
"#;

fn main() {
    let mut jit = JIT::default();
    let main_ptr = jit.compile(SOURCE.trim()).expect("compile failed");

    // The signature of `main` in our language is `fn main() -> i64`.
    let main_fn: extern "C" fn() -> i64 = unsafe { std::mem::transmute(main_ptr) };
    let result = main_fn();
    println!("main() = {result}");
}
