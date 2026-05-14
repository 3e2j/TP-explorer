use tp_explorer::commands;
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
                eprintln!("Usage: tp-explorer export <iso_path> <output_dir>");
                return;
            }
            commands::export::run(&args[2], &args[3])
        }
        "build" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: tp-explorer build <iso_path> <mod_dir> [output_dir] [--iso-output <iso_output>]"
                );
                return;
            }

            let mut output_dir: Option<&str> = None;
            let mut iso_output: Option<&str> = None;
            let mut i = 4;

            while i < args.len() {
                match args[i].as_str() {
                    "--iso-output" => {
                        if i + 1 >= args.len() {
                            eprintln!(
                                "Usage: tp-explorer build <iso_path> <mod_dir> [output_dir] [--iso-output <iso_output>]"
                            );
                            return;
                        }
                        iso_output = Some(&args[i + 1]);
                        i += 2;
                    }
                    "--help" | "-h" => {
                        print_usage();
                        return;
                    }
                    value if output_dir.is_none() => {
                        output_dir = Some(value);
                        i += 1;
                    }
                    value if iso_output.is_none() => {
                        iso_output = Some(value);
                        i += 1;
                    }
                    other => {
                        eprintln!("Unexpected argument: {}", other);
                        eprintln!(
                            "Usage: tp-explorer build <iso_path> <mod_dir> [output_dir] [--iso-output <iso_output>]"
                        );
                        return;
                    }
                }
            }

            if output_dir.is_none() && iso_output.is_none() {
                eprintln!(
                    "Usage: tp-explorer build <iso_path> <mod_dir> [output_dir] [--iso-output <iso_output>]"
                );
                return;
            }

            commands::build::run(&args[3], &args[2], output_dir, iso_output)
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
    println!("  tp-explorer export <iso_path> <output_dir>");
    println!("    Extract ISO files into human-readable folder structure");
    println!("    Generates manifest.json for mod resolution");
    println!();
    println!("  tp-explorer build <iso_path> <mod_dir> [output_dir] [--iso-output <iso_output>]");
    println!("    Build a patched ISO from a mod folder and vanilla ISO");
    println!("    With output_dir only: just build");
    println!("    With --iso-output only: build to a temp dir, export ISO, then clean up");
    println!("    With both: build and export");
}
