use std::env;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();

    if target.contains("android") {
        if let Ok(lib_dir) = env::var("PYO3_CROSS_LIB_DIR") {
            println!("cargo:rustc-link-search=native={lib_dir}");
        }

        println!("cargo:rustc-link-lib=dylib=python3.13");
        println!("cargo:rustc-link-arg=-Wl,--no-undefined");
    }
}
