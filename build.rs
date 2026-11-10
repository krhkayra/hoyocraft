use std::{env, fs, path::PathBuf};

fn main() {
    for file in [
        "src/native.rs",
        "native/application.h",
        "native/application.cpp",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    for key in [
        "HOYOCRAFT_QT_MULTIMEDIA_INCLUDE",
        "HOYOCRAFT_QT_MULTIMEDIA_LIB",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    slint_build::compile_with_config(
        "ui/launcher.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("qt".into())
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("Could not compile Slint UI");

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = fs::read_to_string("src/native.rs")
        .unwrap()
        .parse()
        .unwrap();
    let generated = cxx_gen::generate_header_and_cc(source, &cxx_gen::Opt::default()).unwrap();
    fs::write(out.join("native.cxx.h"), generated.header).unwrap();
    fs::write(out.join("native.cxx.cpp"), generated.implementation).unwrap();
    fs::create_dir_all(out.join("rust")).unwrap();
    fs::write(out.join("rust/cxx.h"), cxx_gen::HEADER).unwrap();
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(&out)
        .include("native")
        .file(out.join("native.cxx.cpp"))
        .file("native/application.cpp");
    let qt = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe("Qt6Widgets")
        .expect("Install qt6-qtbase-devel (Qt6Widgets)");
    build.includes(&qt.include_paths);
    let mut libraries = vec![qt];
    let mut relocated_lib = None;
    let mut relocated_multimedia = false;
    if let Some(include) = env::var_os("HOYOCRAFT_QT_MULTIMEDIA_INCLUDE") {
        build.include(include);
        relocated_lib = env::var_os("HOYOCRAFT_QT_MULTIMEDIA_LIB");
        relocated_multimedia = true;
    } else {
        let multimedia = pkg_config::Config::new()
            .cargo_metadata(false)
            .probe("Qt6Multimedia")
            .expect("Install qt6-qtmultimedia-devel");
        build.includes(&multimedia.include_paths);
        libraries.push(multimedia);
    }
    build.compile("hoyocraft_native");
    // Keep dependent dynamic libraries after the native static archive for
    // linkers using --as-needed.
    if let Some(lib) = relocated_lib {
        println!(
            "cargo:rustc-link-search=native={}",
            PathBuf::from(lib).display()
        );
    }
    if relocated_multimedia {
        println!("cargo:rustc-link-lib=Qt6Multimedia");
    }
    for library in libraries {
        for path in library.link_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
        for lib in library.libs {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}
