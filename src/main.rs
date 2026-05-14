use arc_diff::commands;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    let result = match args[1].as_str() {
        "export" => {
            if args.len() < 4 {
                eprintln!("Usage: arc_diff export <iso_path> <output_dir>");
                return;
            }
            commands::export::run(&args[2], &args[3])
        }
        "build" => {
            if args.len() < 5 {
                eprintln!("Usage: arc_diff build <iso_path> <mod_dir> <output_dir> [iso_output]");
                return;
            }
            let iso_output = args.get(5).map(|s| s.as_str());
            commands::build::run(&args[3], &args[2], &args[4], iso_output)
        }
        _ => {
            print_usage();
            return;
        }
    };

    match result {
        Ok(_) => println!("Command completed successfully"),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!("TP Explorer - Twilight Princess Modding Toolchain");
    println!();
    println!("Usage:");
    println!("  arc_diff export <iso_path> <output_dir>");
    println!("    Extract ISO files into human-readable folder structure");
    println!("    Generates manifest.json for mod resolution");
    println!();
    println!("  arc_diff build <iso_path> <mod_dir> <output_dir> [iso_output]");
    println!("    Build a patched ISO from a mod folder and vanilla ISO");
    println!("    Resolves mod files via manifest.json");
    println!("    If iso_output is provided, directly create a patched ISO file at that path");
}
