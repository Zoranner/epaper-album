use std::path::Path;
use std::process::Command;

fn main() {
    if std::env::var("SKIP_FRONTEND_BUILD").is_ok() {
        return;
    }

    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/index.html");
    println!("cargo:rerun-if-changed=web/package.json");
    println!("cargo:rerun-if-changed=web/bun.lock");
    println!("cargo:rerun-if-changed=web/tsconfig.json");
    println!("cargo:rerun-if-changed=web/tsconfig.node.json");
    println!("cargo:rerun-if-changed=web/vite.config.ts");

    if !Path::new("web/node_modules").exists() {
        progress("Installing dependencies...");
        run("bun", &["install", "--frozen-lockfile"], "web");
    }

    progress("Building frontend...");
    run("bun", &["run", "build"], "web");

    progress("Done.");
}

fn run(cmd: &str, args: &[&str], dir: &str) {
    let display = format!("{cmd} {}", args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|error| panic!("Failed to start `{display}`: {error}"));
    if !status.success() {
        panic!("`{display}` failed with {status}");
    }
}

fn progress(message: &str) {
    println!("cargo:warning=[frontend] {message}");
}
