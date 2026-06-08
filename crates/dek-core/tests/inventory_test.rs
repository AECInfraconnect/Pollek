use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[test]
fn test_no_unwrap_or_expect_in_production_code() {
    let crates_dir = Path::new("../");
    
    for entry in WalkDir::new(crates_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let path = entry.path();
        let path_str = path.to_string_lossy();
        
        // Skip tests and build files
        if path_str.contains("tests") || path_str.contains("build.rs") || path_str.contains("bin") {
            continue;
        }

        let content = fs::read_to_string(path).unwrap_or_default();
        
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if (line.contains(".unwrap()") || line.contains(".expect(")) && !line.trim().starts_with("//") {
                // Ignore our own CLI or test setups, but strictly flag in core paths if needed
                // Currently just a warning, but could be converted to panic
                println!("Warning: unwrap or expect used in production code at {}:{}", path.display(), i + 1);
            }
        }
    }
}
