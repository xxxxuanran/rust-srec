use std::fs;
use std::path::Path;

fn main() -> std::io::Result<()> {
    // Create test directories if they don't exist
    let test_data_path = Path::new("tests/data");
    if !test_data_path.exists() {
        fs::create_dir_all(test_data_path)?;
    }

    Ok(())
}
