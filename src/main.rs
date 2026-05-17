//! TPMT command-line entry point.
//!
//! The binary dispatches `export`, `build`, and their CLI arguments into the
//! library command pipeline.

use std::{env, process};
use tpmt::commands;

/// Program entry-point
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {}", error);
        process::exit(1);
    }
}

/// Command-line inputs -> operations
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match command {
        "export" => run_export(&args[1..]),
        "build" => run_build(&args[1..]),
        _ => {
            print_usage();
            Ok(())
        }
    }
}

/// Exports and converts ISO contents to a folder of choosing in human-readable format.
///
/// See below for more info on file structure.
///
/// ---
#[doc = include_str!("../docs/file-structure.md")]
fn run_export(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        print_export_usage();
        return Ok(());
    }

    commands::export::run(&args[0], &args[1])
}

/// Builds a mod directory into a ready-to-patch ISO format replicating the
/// games original structure. Optionally patches the "build" into the ISO
/// and exports this to an ISO-output.
///
/// Original ISO is left unchanged unless specified as the export path.
fn run_build(args: &[String]) -> Result<(), String> {
    let Some(options) = parse_build_options(args) else {
        print_build_usage();
        return Ok(());
    };

    commands::build::run(
        options.mod_dir,
        options.iso_path,
        options.output_dir,
        options.iso_output,
    )
}

struct BuildOptions<'a> {
    iso_path: &'a str,
    mod_dir: &'a str,
    output_dir: Option<&'a str>,
    iso_output: Option<&'a str>,
}

fn parse_build_options(args: &[String]) -> Option<BuildOptions<'_>> {
    if args.len() < 2 {
        return None;
    }

    let iso_path = args[0].as_str();
    let mod_dir = args[1].as_str();
    let mut output_dir = None;
    let mut iso_output = None;
    let mut i = 2;

    while i < args.len() {
        let value = args[i].as_str();
        match value {
            "--help" | "-h" => return None,
            "--iso-output" => {
                let Some(next) = args.get(i + 1) else {
                    return None;
                };
                iso_output = Some(next.as_str());
                i += 2;
            }
            _ if output_dir.is_none() => {
                output_dir = Some(value);
                i += 1;
            }
            _ if iso_output.is_none() => {
                iso_output = Some(value);
                i += 1;
            }
            _ => return None,
        }
    }

    Some(BuildOptions {
        iso_path,
        mod_dir,
        output_dir,
        iso_output,
    })
}

fn print_usage() {
    println!("TPMT - Twilight Princess Modding Toolchain");
    println!();
    println!("Usage:");
    println!("  tpmt export <iso_path> <output_dir>");
    println!("  tpmt build <iso_path> <mod_dir> [output_dir] [--iso-output <iso_output>]");
}

fn print_export_usage() {
    eprintln!(
        "Export game assets from an ISO to a directory

        Usage: tpmt export <ISO_PATH> <OUTPUT_DIR>

        Arguments:
        <ISO_PATH>      Path to the input ISO file
        <OUTPUT_DIR>    Directory to write exported assets into

        Options:
        -h, --help  Print help"
    );
}

fn print_build_usage() {
    eprintln!(
        "Build a mod and optionally repack it into an ISO

        Usage: tpmt build <ISO_PATH> <MOD_DIR> [OUTPUT_DIR] [OPTIONS]

        Arguments:
        <ISO_PATH>      Path to the source ISO file
        <MOD_DIR>       Directory containing the mod files to build
        [OUTPUT_DIR]    Directory to write build output (default: current directory)

        Options:
            --iso-output <ISO_OUTPUT>  Path for the repacked ISO output file
        -h, --help                     Print help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    // Verifies the build parser accepts the documented positional arguments.
    #[test]
    fn parse_build_options_accepts_required_arguments() {
        assert!(parse_build_options(&args(&["iso.iso", "mod_dir"])).is_some());
    }

    // Verifies optional output and ISO output flags are captured in order.
    #[test]
    fn parse_build_options_parses_optional_outputs() {
        let binding = args(&["iso.iso", "mod_dir", "out", "--iso-output", "patched.iso"]);
        let parsed = parse_build_options(&binding).map(|o| (o.output_dir, o.iso_output));
        assert_eq!(parsed, Some((Some("out"), Some("patched.iso"))));
    }

    // Verifies help flags short-circuit the build parser instead of treating them as paths.
    #[test]
    fn parse_build_options_rejects_help_flag() {
        assert!(parse_build_options(&args(&["iso.iso", "mod_dir", "--help"])).is_none());
    }

    // Verifies missing required arguments return no build configuration.
    #[test]
    fn parse_build_options_rejects_missing_paths() {
        assert!(parse_build_options(&args(&["iso.iso"])).is_none());
    }
}
