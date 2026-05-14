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

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(
    mod_dir: &str,
    iso_path: &str,
    output_dir: Option<&str>,
    iso_output: Option<&str>,
) -> Result<(), String> {
    println!("Building mod: {}", mod_dir);
    println!("Source ISO: {}", iso_path);
    if let Some(iso_out) = iso_output {
        println!("ISO output: {}", iso_out);
    }

    let mod_path = Path::new(mod_dir);
    let iso_path_p = Path::new(iso_path);

    let (output_path_buf, temp_output_dir) = match output_dir {
        Some(dir) => {
            let path = PathBuf::from(dir);
            std::fs::create_dir_all(&path)
                .map_err(|e| format!("Create output dir failed: {}", e))?;
            println!("Output directory: {}", path.display());
            (path, None)
        }
        None => {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Create temp dir failed: {}", e))?
                .as_millis();
            let path = std::env::temp_dir()
                .join(format!("tp-explorer-build-{unique}-{}", std::process::id()));
            std::fs::create_dir_all(&path)
                .map_err(|e| format!("Create temp output dir failed: {}", e))?;
            println!("Output directory: {} (temporary)", path.display());
            (path, Some(()))
        }
    };
    let output_path = output_path_buf.as_path();

    // Stage 1: Load manifest and compare hashes
    let modified_files = hash_check::find_modified_files(mod_path)?;
    println!("Found {} modified files", modified_files.len());

    // Stage 2: Compile modified files to original formats
    let compiled = compile::compile_modified_files(&modified_files, mod_path)?;
    println!("Compiled {} files", compiled.len());

    // Stage 3-5: Resolve .arc dependencies, extract, and assemble
    let result = assemble::build_archives(&compiled, iso_path_p, mod_path, output_path, iso_output);

    if let Some(()) = temp_output_dir {
        let _ = std::fs::remove_dir_all(output_path);
    }

    result?;
    if iso_output.is_some() && output_dir.is_none() {
        println!("Build complete. Temporary build output removed after ISO export.");
    } else {
        println!(
            "Build complete. Output written to {}",
            output_path.display()
        );
    }
    Ok(())
}
