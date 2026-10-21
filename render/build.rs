use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

struct ShaderTarget {
    src: &'static str,
    entry: &'static str,
    stage: &'static str,
    dst: &'static str,
}

fn main() {
    println!("cargo:rerun-if-changed=shaders");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable is not set");
    let out_path = PathBuf::from(out_dir);

    let exe_name = if cfg!(target_os = "windows") { "slangc.exe" } else { "slangc" };

    let slangc_path = match env::var("SLANG_DIR") {
        Ok(dir) => Path::new(&dir).join("bin").join(exe_name),
        Err(_) => PathBuf::from(exe_name),
    };
    
    let shaders = [
        ShaderTarget { src: "shaders/main.slang", entry: "vs_main", stage: "vertex", dst: "main_vs.wgsl" },
        ShaderTarget { src: "shaders/main.slang", entry: "fs_main", stage: "fragment", dst: "main_fs.wgsl" },
        ShaderTarget { src: "shaders/main.slang", entry: "depth_fs_main", stage: "fragment", dst: "depth_fs.wgsl" },
        ShaderTarget { src: "shaders/shadow.slang", entry: "vs_main", stage: "vertex", dst: "shadow_vs.wgsl" },
        ShaderTarget { src: "shaders/cull.slang", entry: "cull_lights", stage: "compute", dst: "cull_cs.wgsl" },
        ShaderTarget { src: "shaders/postprocess.slang", entry: "vs_main", stage: "vertex", dst: "postprocess_vs.wgsl" },
        ShaderTarget { src: "shaders/postprocess.slang", entry: "fs_main", stage: "fragment", dst: "postprocess_fs.wgsl" },
        ShaderTarget { src: "shaders/bloom.slang", entry: "vs_main", stage: "vertex", dst: "bloom_vs.wgsl" },
        ShaderTarget { src: "shaders/bloom.slang", entry: "fs_downsample", stage: "fragment", dst: "bloom_down_fs.wgsl" },
        ShaderTarget { src: "shaders/bloom.slang", entry: "fs_upsample", stage: "fragment", dst: "bloom_up_fs.wgsl" },
        ShaderTarget { src: "shaders/blit.slang", entry: "vs_main", stage: "vertex", dst: "blit_vs.wgsl" },
        ShaderTarget { src: "shaders/blit.slang", entry: "fs_main", stage: "fragment", dst: "blit_fs.wgsl" },
        ShaderTarget { src: "shaders/wireframe.slang", entry: "fs_main", stage: "fragment", dst: "wireframe_fs.wgsl" },
        ShaderTarget { src: "shaders/wireframe.slang", entry: "vs_main", stage: "vertex", dst: "wireframe_vs.wgsl" },
        ShaderTarget { src: "shaders/outline.slang", entry: "fs_main", stage: "fragment", dst: "outline_fs.wgsl" },
        ShaderTarget { src: "shaders/outline.slang", entry: "vs_main", stage: "vertex", dst: "outline_vs.wgsl" },
    ];

    for target in &shaders {
        let output_file = out_path.join(target.dst);

        let status = Command::new(&slangc_path)
            .arg(target.src)
            .arg("-target").arg("wgsl")
            .arg("-entry").arg(target.entry)
            .arg("-stage").arg(target.stage)
            .arg("-o").arg(&output_file)
            .status()
            .unwrap_or_else(|err| panic!("Failed to start slangc compiler ({:?}): {}", slangc_path, err));

        assert!(
            status.success(),
            "Shader compilation failed for '{}' (entry: '{}', stage: '{}')",
            target.src,
            target.entry,
            target.stage
        );
    }
}