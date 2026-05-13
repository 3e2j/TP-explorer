/*
Compile command: Takes a modder's sparse directory and patches a vanilla ISO.

This command:
1. Reads manifest.json to resolve mod files to their ISO paths
2. For files in .arc archives: extracts the full .arc, patches changed internal files, repacks
3. Generates a new ISO with only the changed files
4. Everything unchanged is sourced from the vanilla ISO at runtime
*/

pub fn run(_mod_dir: &str, _vanilla_iso: &str, _output_iso: &str) -> Result<(), String> {
    Err("compile command not yet implemented".to_string())
}
