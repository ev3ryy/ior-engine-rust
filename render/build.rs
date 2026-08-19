use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=shaders");

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(out_dir);

    let slangc_path = match env::var("SLANG_DIR") {
        Ok(dir) => {
            let mut path = PathBuf::from(dir);
            path.push("bin");
            path.push("slangc");
            path
        }
        Err(_) => PathBuf::from("slangc"),
    };

    let shaders = vec![
        ("shaders/main.slang", "vs_main", "vertex", "main_vs.wgsl"),
        ("shaders/main.slang", "fs_main", "fragment", "main_fs.wgsl"),
        ("shaders/main.slang", "depth_fs_main", "fragment", "depth_fs.wgsl"),
        ("shaders/blit.slang", "vs_main", "vertex", "blit_vs.wgsl"),
        ("shaders/blit.slang", "fs_main", "fragment", "blit_fs.wgsl"),
        ("shaders/cull.slang", "cull_lights", "compute", "cull_cs.wgsl"),
    ];

    for (src, entry, stage, dst) in shaders {
        let output_file = out_path.join(dst);
        
        let status = Command::new(&slangc_path)
            .arg(src)
            .arg("-target").arg("wgsl")
            .arg("-entry").arg(entry)
            .arg("-stage").arg(stage)
            .arg("-o").arg(&output_file)
            .status()
            .expect("Failed to start slangc");

        assert!(status.success(), "Shader compilation error: {}", src);
    }
}