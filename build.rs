use anyhow::*;
use fs_extra::copy_items;
use fs_extra::dir::CopyOptions;

fn main() -> Result<()> {
    // This tells cargo to rerun this script if something in /res/ changes.
    println!("cargo:rerun-if-changed=./res/*");

    // Copy the /res/ folder to the output directory.
    let mut copy_options = CopyOptions::new();
    copy_options.overwrite = true;
    let paths_to_copy = vec!["./res/"];

    // Make sure the directories exist.
    std::fs::create_dir_all("./target/debug/")?;
    std::fs::create_dir_all("./target/release/")?;

    // Check if we're in debug or release mode.
    if cfg!(debug_assertions) {
        // Copy the /res/ folder to the output directory.
        copy_items(&paths_to_copy, "./target/debug/", &copy_options)?;
    } else {
        // Copy the /res/ folder to the output directory.
        copy_items(&paths_to_copy, "./target/release/", &copy_options)?;
    }

    Ok(())
}
