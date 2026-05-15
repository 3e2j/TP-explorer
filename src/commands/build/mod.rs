/*
Build command: reverse of export.

Multi-stage pipeline:
1) Load manifest and compare file hashes (original vs mod)
2) Compile modified files (JSON→BMG, etc.)
3) Buffer direct files
4) Resolve archive-backed files and fetch missing archive data
5) Assemble modified archives
6) Write the full build output
*/

mod assemble;
mod archive_plan;
mod compile;
mod hash_check;
mod output;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(
    mod_dir: &str,
    iso_path: &str,
    output_dir: Option<&str>,
    iso_output: Option<&str>,
) -> Result<(), String> {
    let build_temp: bool = output_dir.is_none();
    let build_iso: bool = iso_output.is_some();

    println!("Building mod: {}", mod_dir);
    println!("Source ISO: {}", iso_path);
    if let Some(iso_out) = iso_output {
        println!("ISO output: {}", iso_out);
    }

    let mod_path = Path::new(mod_dir);
    let iso_path_p = Path::new(iso_path);

    let (output_path_buf, _temp_output_dir) = match output_dir {
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

    let (direct_files, archive_files): (Vec<_>, Vec<_>) = compiled
        .into_iter()
        .partition(|c| c.mod_file.archive.is_none());

    // Stage 3: Buffer direct (non-archive) outputs
    let direct_outputs = output::collect_direct_outputs(&direct_files);

    // Stage 4: Read the manifest archive map and fetch any missing archive
    // contents from the ISO when needed.
    let archive_inputs = archive_plan::plan_archive_inputs(&archive_files, mod_path, iso_path_p)?;

    // Stage 5: Assemble modified archives
    let archive_outputs = assemble::assemble_archives(&archive_inputs)?;

    let mut build_outputs = direct_outputs;
    build_outputs.extend(archive_outputs);

    // Stage 6: Write build to output
    let build = output::write_outputs(output_path, &build_outputs)?;

    // Rebuild ISO if requested
    if build_iso {
        let iso_out = iso_output.expect("build_iso set but iso_output missing");
        let arc_paths: Vec<String> = build_outputs
            .iter()
            .filter(|o| o.path.ends_with(".arc"))
            .map(|o| o.path.clone())
            .collect();
        output::rebuild_iso_from_outputs(iso_path_p, output_path, &arc_paths, &build, iso_out)?;
    }

    if build_temp {
        let _ = std::fs::remove_dir_all(output_path);
    }

    if build_temp {
        println!("Build complete.");
    } else {
        println!(
            "Build complete. Output folder written to {}",
            output_path.display()
        );
    }
    Ok(())
}
