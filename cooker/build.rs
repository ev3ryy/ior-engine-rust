fn main() {
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=dylib=stdc++");

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=dylib=c++");
}