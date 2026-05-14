/*
Build command: reverse of export.

Multi-stage pipeline:
1) Load manifest and compare file hashes (original vs mod)
2) Compile modified files (JSON→BMG, etc.)
3) Resolve dependencies from manifest (all files per arc)
4) Extract dependencies from ISO
5) Assemble modified archives
6) Output final build directory
*/

mod assemble;
mod compile;
mod hash_check;

use std::path::Path;

pub fn run(mod_dir: &str, iso_path: &str, output_dir: &str, iso_output: Option<&str>) -> Result<(), String> {
    std::fs::create_dir_all(output_dir).map_err(|e| format!("Create output dir failed: {}", e))?;

    println!("Building mod: {}", mod_dir);
    println!("Source ISO: {}", iso_path);
    println!("Output directory: {}", output_dir);
    if let Some(iso_out) = iso_output {
        println!("ISO output: {}", iso_out);
    }

    let mod_path = Path::new(mod_dir);
    let iso_path_p = Path::new(iso_path);
    let output_path = Path::new(output_dir);

    // Stage 1: Load manifest and compare hashes
    let modified_files = hash_check::find_modified_files(mod_path)?;
    println!("Found {} modified files", modified_files.len());

    // Stage 2: Compile modified files to original formats
    let compiled = compile::compile_modified_files(&modified_files, mod_path)?;
    println!("Compiled {} files", compiled.len());

    // Stage 3-5: Resolve .arc dependencies, extract, and assemble
    assemble::build_archives(&compiled, iso_path_p, mod_path, output_path, iso_output)?;

    println!(
        "Build complete. Output written to {}",
        output_path.display()
    );
    Ok(())
}
