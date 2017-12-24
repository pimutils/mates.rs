extern crate clap;

use std::env;
use std::fs;

use clap::Shell;

#[path = "src/mates/app.rs"]
mod app;

fn main() {
    let outdir = match env::var_os("OUT_DIR") {
        None => return,
        Some(outdir) => outdir,
    };
    fs::create_dir_all(&outdir).unwrap();

    let mut app = app::app();
    app.gen_completions("mates", Shell::Bash, &outdir);
    app.gen_completions("mates", Shell::Fish, &outdir);
    app.gen_completions("mates", Shell::Zsh, &outdir);
}
