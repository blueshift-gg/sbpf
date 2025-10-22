use {
    anyhow::{Error, Result},
    std::{fs, path::Path},
};

pub fn clean() -> Result<(), Error> {
    fs::remove_dir_all(".sbpf")?;
    clean_directory("deploy", "so")?;
    Ok(())
}

fn clean_directory(directory: &str, extension: &str) -> Result<(), Error> {
    let path = Path::new(directory);
    for entry in path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension().and_then(|ext| ext.to_str())
            && (extension.is_empty() || ext == extension)
        {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}
