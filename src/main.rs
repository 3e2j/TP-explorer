use arc_diff::arc_extractor;
use arc_diff::exporter;
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "export-bmg" => {
            if args.len() < 4 {
                eprintln!("Usage: arc_diff export-bmg <iso_path> <output_dir>");
                return;
            }
            let iso_path = &args[2];
            let output_dir = &args[3];

            println!("Exporting BMG files from {} to {}", iso_path, output_dir);
            match exporter::export_bmg_from_iso(iso_path, output_dir) {
                Ok(exported) => {
                    println!("Successfully exported {} BMG files", exported.len());
                }
                Err(e) => {
                    eprintln!("Export failed: {}", e);
                }
            }
        }
        "extract-arc" => {
            if args.len() < 4 {
                eprintln!("Usage: arc_diff extract-arc <input_dir> <output_dir>");
                return;
            }
            let input_dir = &args[2];
            let output_dir = &args[3];

            println!("Extracting ARC files from {} to {}", input_dir, output_dir);
            match arc_extractor::extract_arc_files(input_dir, output_dir) {
                Ok(files) => {
                    println!("Successfully extracted {} files", files.len());
                }
                Err(e) => {
                    eprintln!("Extraction failed: {}", e);
                }
            }
        }
        "diff" => {
            if args.len() < 3 {
                eprintln!("Usage: arc_diff diff <iso_path> <folder_path>");
                return;
            }
            let iso_path = PathBuf::from(&args[2]);
            let folder_path = PathBuf::from(&args[3]);

            println!(
                "Comparing ISO {} against folder {}...",
                iso_path.display(),
                folder_path.display()
            );

            match arc_diff::diff::diff_iso_files_against_folder(&iso_path, &folder_path) {
                Ok(result) => println!("{}", result),
                Err(err) => eprintln!("Error: {}", err),
            }
        }
        _ => {
            print_usage();
        }
    }
}

fn print_usage() {
    println!("Usage:");
    println!("  arc_diff test-bmg <bmg_file>");
    println!("    Parse and output BMG file as JSON");
    println!();
    println!("  arc_diff export-bmg <iso_path> <output_dir>");
    println!("    Export BMG files from ISO to JSON in output directory");
    println!();
    println!("  arc_diff extract-arc <input_dir> <output_dir>");
    println!("    Extract all ARC files in directory, preserving internal structure");
    println!();
    println!("  arc_diff diff <iso_path> <folder_path>");
    println!("    Compare ISO files against folder");
}
