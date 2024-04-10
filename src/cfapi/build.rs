fn main() -> miette::Result<()> {
    let path = std::path::PathBuf::from("wrapper/include"); // include path
    let path_cfapi_include = std::path::PathBuf::from("cfapi-cpp-linux-1.6.0.0/include");
    // let mut b =
    //     autocxx_build::Builder::new("src/main.rs", &[&path, &path_cfapi_include]).build()?;
    let mut b =
        autocxx_build::Builder::new("src/binding.rs", &[&path, &path_cfapi_include]).build()?;
    // This assumes all your C++ bindings are in main.rs
    b.flag_if_supported("-std=c++14")
        // .link_lib_modifier("-l dylib=sample")
        .cpp_link_stdlib("pthread")
        .file("wrapper/src/api.cc")
        .compile("autocxx-cfapi"); // arbitrary library name, pick anything
   
    println!("cargo:rerun-if-changed=src/binding.rs");
    println!("cargo:rerun-if-changed=src/user_event.rs");
    println!("cargo:rerun-if-changed=src/session_event.rs");
    println!("cargo:rerun-if-changed=src/message_event.rs");
    println!("cargo:rerun-if-changed=wrapper/src/api.cc");
    println!("cargo:rerun-if-changed=wrapper/include/api.h");
    // println!("cargo:rustc-link-search=./cfapi-cpp-linux-1.6.0.0/lib");
    // println!("cargo:rustc-link-search=static=./cfapi-cpp-linux-1.6.0.0/lib");
    println!("cargo:rustc-link-search=native=./cfapi-cpp-linux-1.6.0.0/lib");
    println!("cargo:rustc-link-search=native=./src/cfapi/cfapi-cpp-linux-1.6.0.0/lib");
    println!("cargo:rustc-link-lib=dylib=cfapi");
    println!("cargo:rustc-link-search=native=./cfapi-cpp-linux-1.6.0.0/dbcapi");
    println!("cargo:rustc-link-search=native=./src/cfapi/cfapi-cpp-linux-1.6.0.0/dbcapi");
    println!("cargo:rustc-link-lib=dylib=dbcapi64");
    println!("cargo:rustc-link-lib=dylib=md564");
    println!("cargo:rustc-link-lib=dylib=pkware64");
    println!("cargo:rustc-link-lib=dylib=port64");
    // Add instructions to link to any C++ libraries you need.
    Ok(())
}
