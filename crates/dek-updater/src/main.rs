use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("Pollen DEK Auto-Updater");

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dek-updater <target_exe> <new_exe>");
        std::process::exit(1);
    }

    let target_exe = PathBuf::from(&args[1]);
    let new_exe = PathBuf::from(&args[2]);

    if !new_exe.exists() {
        eprintln!("New executable not found: {:?}", new_exe);
        std::process::exit(1);
    }

    // On Windows, you can rename an executing file, but you can't delete or overwrite it directly.
    let backup_exe = target_exe.with_extension("exe.bak");

    // Remove old backup if it exists
    if backup_exe.exists()
        && let Err(e) = fs::remove_file(&backup_exe) {
            eprintln!("Failed to remove old backup: {e}");
        }

    // Rename current running executable to backup
    if target_exe.exists()
        && let Err(e) = fs::rename(&target_exe, &backup_exe) {
            eprintln!("Failed to rename active executable: {e}");
            std::process::exit(1);
        }

    // Rename the new downloaded executable to the target name
    if let Err(e) = fs::rename(&new_exe, &target_exe) {
        eprintln!("Failed to move new executable into place: {e}");
        // Rollback
        if backup_exe.exists() {
            let _ = fs::rename(&backup_exe, &target_exe);
        }
        std::process::exit(1);
    }

    println!("Update successful. Please restart the service.");
}
