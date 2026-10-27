use std::{fs, io::Read, path::Path};

use anyhow::Result;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

// Materialize links so a copied skill remains usable without its source folder.
// WalkDir detects directory-link cycles and returns an error.
pub(crate) fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in WalkDir::new(source).follow_links(true) {
        let entry = entry?;
        let target = destination.join(entry.path().strip_prefix(source)?);
        if entry.file_type().is_dir() {
            fs::create_dir_all(target)?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub(crate) fn tree_hash(root: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    for entry in WalkDir::new(root).follow_links(true).sort_by_file_name() {
        let entry = entry?;
        let relative = entry.path().strip_prefix(root)?;
        let name = relative.to_string_lossy();
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        if entry.file_type().is_dir() {
            hash.update(b"d");
        } else if entry.file_type().is_file() {
            hash.update(b"f");
            let mut file = fs::File::open(entry.path())?;
            hash.update(file.metadata()?.len().to_le_bytes());
            let mut buffer = [0; 64 * 1024];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_tracks_content_paths_and_empty_folders_but_not_timestamps() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("source");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a"), b"abc").unwrap();
        let original = tree_hash(&root).unwrap();
        fs::write(root.join("a"), b"abc").unwrap();
        assert_eq!(tree_hash(&root).unwrap(), original);
        let copy = temp.path().join("copy");
        copy_tree(&root, &copy).unwrap();
        assert_eq!(tree_hash(&copy).unwrap(), original);
        fs::write(root.join("a"), b"abd").unwrap();
        assert_ne!(tree_hash(&root).unwrap(), original);
        fs::write(root.join("a"), b"abc").unwrap();
        fs::rename(root.join("a"), root.join("b")).unwrap();
        assert_ne!(tree_hash(&root).unwrap(), original);
        fs::rename(root.join("b"), root.join("a")).unwrap();
        fs::create_dir(root.join("empty")).unwrap();
        assert_ne!(tree_hash(&root).unwrap(), original);
    }
}
