fn main() {
    linker_be_nice();
    // build_and_gen_bind_ffi_code();
    // cc crate does not properly link the library with
    // the use of linkall.x below, so do it manually.
    // println!("cargo:rustc-link-arg=-ldr_flac");
    // make sure linkall.x is the last linker script (otherwise might cause problems with flip-link)
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}

fn build_and_gen_bind_ffi_code() {
    cc::Build::new()
        // .compiler("xtensa-esp32s3-none-elf")
        .include("vendor/dr_libs")
        .define("DR_FLAC_NO_STDIO", None)
        .define("DR_FLAC_IMPLEMENTATION", None)
        .file("vendor/dr_flac.c")
        .compile("dr_flac");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    bindgen::Builder::default()
        .clang_arg("--target=xtensa-esp32s3-none-elf")
        .clang_arg("-fretain-comments-from-system-headers")
        .generate_comments(true)
        .ctypes_prefix("cty")
        // The input header we would like to generate
        // bindings for.
        .header("vendor/dr_libs/dr_flac.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        // .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .use_core()
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings")
        .write_to_file(
            std::path::PathBuf::from(
                // std::env::var("OUT_DIR").unwrap()
                "src/audio/codec/",
            )
            .join("dr_flac_bindings.rs"),
        )
        .unwrap();
    // println!("cargo:rerun-if-changed=bindgen.h");
    // println!("cargo:rerun-if-changed=minimp3.c");
}

fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                what if what.starts_with("_defmt_") => {
                    eprintln!();
                    eprintln!(
                        "💡 `defmt` not found - make sure `defmt.x` is added as a linker script and you have included `use defmt_rtt as _;`"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("💡 Is the linker script `linkall.x` missing?");
                    eprintln!();
                }
                what if what.starts_with("esp_rtos_") => {
                    eprintln!();
                    eprintln!(
                        "💡 `esp-radio` has no scheduler enabled. Make sure you have initialized `esp-rtos` or provided an external scheduler."
                    );
                    eprintln!();
                }
                "embedded_test_linker_file_not_added_to_rustflags" => {
                    eprintln!();
                    eprintln!(
                        "💡 `embedded-test` not found - make sure `embedded-test.x` is added as a linker script for tests"
                    );
                    eprintln!();
                }
                "free"
                | "malloc"
                | "calloc"
                | "get_free_internal_heap_size"
                | "malloc_internal"
                | "realloc_internal"
                | "calloc_internal"
                | "free_internal" => {
                    eprintln!();
                    eprintln!(
                        "💡 Did you forget the `esp-alloc` dependency or didn't enable the `compat` feature on it?"
                    );
                    eprintln!();
                }
                _ => (),
            },
            // we don't have anything helpful for "missing-lib" yet
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=-Wl,--error-handling-script={}",
        std::env::current_exe().unwrap().display()
    );
}
