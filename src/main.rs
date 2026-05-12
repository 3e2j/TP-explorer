use arc_diff::diff;
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: arc_diff <iso_path> <folder_path>");
        return;
    }

    let iso_path = PathBuf::from(&args[1]);
    let folder_path = PathBuf::from(&args[2]);

    println!(
        "Comparing ISO {} against folder {}...",
        iso_path.display(),
        folder_path.display()
    );

    match diff::diff_iso_files_against_folder(&iso_path, &folder_path) {
        Ok(result) => println!("{}", result),
        Err(err) => eprintln!("Error: {}", err),
    }
}
