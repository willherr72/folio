fn main() {
    build_jpeg();
    tauri_build::build()
}

fn build_jpeg() {
    use std::{env, path::PathBuf, process::Command};
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = root.join("vendor/libjpeg-turbo-3.2.1");
    let native = root.join("native/jpeg");
    let build = output.join("jpeg-build");
    let library = output.join("jpeg-lib");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", native.display());
    println!("cargo:rerun-if-env-changed=CMAKE");
    let cmake = env::var_os("CMAKE").unwrap_or_else(|| "cmake".into());
    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&build)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DENABLE_SHARED=OFF",
            "-DWITH_TURBOJPEG=OFF",
            "-DWITH_SIMD=OFF",
            "-DWITH_ARITH_DEC=OFF",
            "-DWITH_ARITH_ENC=OFF",
        ])
        .arg(format!(
            "-DCMAKE_PROJECT_INCLUDE={}",
            native.join("configure.cmake").display()
        ))
        .arg(format!("-DFOLIO_NATIVE_DIR={}", native.display()))
        .arg(format!("-DFOLIO_LIBRARY_DIR={}", library.display()));
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
            "x86_64" => "x64",
            "aarch64" => "ARM64",
            "x86" => "Win32",
            other => panic!("Unsupported JPEG build target: {other}"),
        };
        let static_crt = env::var("CARGO_CFG_TARGET_FEATURE")
            .unwrap_or_default()
            .split(',')
            .any(|v| v == "crt-static");
        configure
            .args(["-G", "Visual Studio 17 2022", "-A", arch])
            .arg(if static_crt {
                "-DWITH_CRT_DLL=OFF"
            } else {
                "-DWITH_CRT_DLL=ON"
            });
    } else {
        assert_eq!(
            env::var("HOST").unwrap(),
            env::var("TARGET").unwrap(),
            "JPEG cross-compilation needs an explicit CMake toolchain"
        );
        configure.args(["-G", "Ninja", "-DCMAKE_POSITION_INDEPENDENT_CODE=ON"]);
    }
    assert!(
        configure
            .status()
            .expect("CMake is required for the native JPEG codec")
            .success(),
        "JPEG codec configuration failed"
    );
    assert!(
        Command::new(cmake)
            .arg("--build")
            .arg(&build)
            .args([
                "--config",
                "Release",
                "--target",
                "jpeg-static",
                "--parallel",
                "2"
            ])
            .status()
            .expect("Could not build JPEG codec")
            .success(),
        "JPEG codec build failed"
    );
    println!("cargo:rustc-link-search=native={}", library.display());
    println!("cargo:rustc-link-lib=static=folio_jpeg");
}
