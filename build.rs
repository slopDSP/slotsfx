fn npm_executable() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn main() {
    let ui_src = std::path::Path::new("ui_web");
    if ui_src.exists() {
        let mut cmd = std::process::Command::new(npm_executable());
        cmd.arg("run").arg("build").current_dir(ui_src);

        match cmd.status() {
            Ok(status) if status.success() => {
                println!("cargo:warning=ui_web build completed successfully");
            }
            Ok(status) => println!(
                "cargo:warning=ui_web build failed (status {}) — plugin UI will be stale or missing",
                status
            ),
            Err(err) => println!(
                "cargo:warning=ui_web build failed ({}) — plugin UI will be stale or missing",
                err
            ),
        }
    }
    println!("cargo:rerun-if-changed=ui_web/src/main.js");
    println!("cargo:rerun-if-changed=ui_web/src/style.css");
    println!("cargo:rerun-if-changed=ui_web/index.html");
}
