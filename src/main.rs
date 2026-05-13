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
        // "build" => {
        //     if args.len() < 5 {
        //         eprintln!("Usage: arc_diff compile <mod_dir> <vanilla_iso> <output_folder>");
        //         return;
        //     }
        //     commands::build::run(&args[2], &args[3], &args[4])
        // }
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
    println!("  arc_diff compile <mod_dir> <vanilla_iso> <output_iso>");
    println!("    Build a patched ISO from a mod folder and vanilla ISO");
    println!("    Resolves mod files via manifest.json");
}
