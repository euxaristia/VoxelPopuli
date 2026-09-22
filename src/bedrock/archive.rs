//! Checked .mcworld archive handling.
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const MAX_FILES: usize = 100_000;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WORLD_BYTES: u64 = 8 * 1024 * 1024 * 1024;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn relative_path(name: &str) -> io::Result<PathBuf> {
    if name.is_empty()
        || name.len() > 1024
        || name.contains(['\\', ':', '\0'])
        || name.starts_with('/')
    {
        return Err(invalid("Unsafe archive path"));
    }
    let mut path = PathBuf::new();
    for part in name.strip_suffix('/').unwrap_or(name).split('/') {
        let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| c.is_control())
            || matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (base.len() == 4
                && (base.starts_with("COM") || base.starts_with("LPT"))
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(invalid("Unsafe archive path component"));
        }
        path.push(part);
    }
    Ok(path)
}

/// Extract only into a new destination. Validation happens before any file is written.
pub fn unpack(source: &Path, destination: &Path) -> io::Result<()> {
    let mut archive = ZipArchive::new(fs::File::open(source)?).map_err(io::Error::other)?;
    if archive.len() > MAX_FILES {
        return Err(invalid("Too many world archive entries"));
    }
    let mut names = HashSet::new();
    let mut tree: HashMap<String, (String, bool)> = HashMap::new();
    let mut required = [false; 2];
    let mut paths = Vec::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(io::Error::other)?;
        let path = relative_path(entry.name())?;
        if !names.insert(entry.name().trim_end_matches('/').to_ascii_lowercase()) {
            return Err(invalid("Duplicate or case-colliding archive path"));
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != if entry.is_dir() { 0o040000 } else { 0o100000 } {
                return Err(invalid("World archive contains a special file"));
            }
        }
        let name = entry.name().strip_suffix('/').unwrap_or(entry.name());
        let components: Vec<_> = name.split('/').collect();
        for end in 1..=components.len() {
            let prefix = components[..end].join("/");
            let directory = end < components.len() || entry.is_dir();
            if let Some((spelling, was_directory)) = tree.get(&prefix.to_ascii_lowercase()) {
                if *spelling != prefix || *was_directory != directory {
                    return Err(invalid("Conflicting archive file or directory paths"));
                }
            } else {
                tree.insert(prefix.to_ascii_lowercase(), (prefix, directory));
            }
        }
        if !entry.is_dir() {
            required[0] |= entry.name() == "level.dat";
            required[1] |= entry.name() == "db/CURRENT";
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| invalid("Archive size overflow"))?;
        if entry.size() > MAX_FILE_BYTES || total > MAX_WORLD_BYTES {
            return Err(invalid("World archive exceeds extraction limits"));
        }
        paths.push(path);
    }
    if !required.into_iter().all(|present| present) {
        return Err(invalid(
            "World files must be at the archive root (level.dat and db/CURRENT)",
        ));
    }
    fs::create_dir(destination)?;
    let result = (|| {
        for (i, relative) in paths.iter().enumerate() {
            let mut entry = archive.by_index(i).map_err(io::Error::other)?;
            let path = destination.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&path)?;
                continue;
            }
            fs::create_dir_all(path.parent().unwrap())?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            let expected = entry.size();
            let copied = io::copy(&mut (&mut entry).take(expected + 1), &mut file)?;
            if copied != expected {
                return Err(invalid("Archive entry size mismatch"));
            }
            file.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        // This directory was created exclusively above; archive links were rejected.
        let _ = fs::remove_dir_all(destination);
    }
    result
}

/// The caller must close its database before packaging a coherent world snapshot.
pub fn pack(source: &Path, destination: &Path) -> io::Result<()> {
    if !source.join("level.dat").is_file() || !source.join("db/CURRENT").is_file() {
        return Err(invalid("Not a Bedrock world directory"));
    }
    fn walk(
        root: &Path,
        relative: &Path,
        files: &mut Vec<PathBuf>,
        total: &mut u64,
    ) -> io::Result<()> {
        for entry in fs::read_dir(root.join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if path == Path::new("db").join("LOCK") {
                continue;
            }
            let metadata = entry.metadata()?;
            if entry.file_type()?.is_symlink() {
                return Err(invalid("World contains a symlink"));
            }
            if metadata.is_dir() {
                walk(root, &path, files, total)?;
            } else if metadata.is_file() {
                *total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| invalid("World size overflow"))?;
                if metadata.len() > MAX_FILE_BYTES
                    || *total > MAX_WORLD_BYTES
                    || files.len() >= MAX_FILES
                {
                    return Err(invalid("World exceeds archive limits"));
                }
                files.push(path);
            } else {
                return Err(invalid("World contains a special file"));
            }
        }
        Ok(())
    }
    let source = source.canonicalize()?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if parent.canonicalize()?.starts_with(&source) {
        return Err(invalid("Archive destination is inside the world"));
    }
    let mut files = Vec::new();
    walk(&source, Path::new(""), &mut files, &mut 0)?;
    files.sort();
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
        let mut archive = ZipWriter::new(file);
        for relative in files {
            let name = relative
                .to_str()
                .ok_or_else(|| invalid("World path is not UTF-8"))?
                .replace('\\', "/");
            relative_path(&name)?;
            archive.start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )?;
            io::copy(&mut fs::File::open(source.join(relative))?, &mut archive)?;
        }
        archive.finish()?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn reject_inconsistent_archive_trees_before_extraction() {
        let root = std::env::temp_dir().join(format!(
            "voxel-archive-invalid-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir(&root).unwrap();
        for (index, entries) in [
            vec!["level.dat/", "db/CURRENT"],
            vec!["LEVEL.DAT", "db/CURRENT"],
            vec!["level.dat", "db/CURRENT", "DB/extra"],
            vec!["level.dat", "db/CURRENT", "db/file", "db/file/child"],
            vec!["level.dat", "db/CURRENT", "db//"],
        ]
        .iter()
        .enumerate()
        {
            let path = root.join(format!("{index}.mcworld"));
            let mut archive = ZipWriter::new(fs::File::create(&path).unwrap());
            for name in entries {
                if name.ends_with('/') {
                    archive
                        .add_directory(*name, SimpleFileOptions::default())
                        .unwrap();
                } else {
                    archive
                        .start_file(*name, SimpleFileOptions::default())
                        .unwrap();
                    archive.write_all(b"data").unwrap();
                }
            }
            archive.finish().unwrap();
            let dst = root.join(format!("out-{index}"));
            assert!(unpack(&path, &dst).is_err(), "accepted {entries:?}");
            assert!(!dst.exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn archive_paths_are_portable_and_cannot_escape() {
        for bad in [
            "../escape",
            "/absolute",
            "C:/escape",
            "db/../level.dat",
            "db\\evil",
            "db/file:stream",
            "CON",
            "db/nul.txt",
            "db/a.",
            "db/a ",
            "db//a",
        ] {
            assert!(relative_path(bad).is_err(), "{bad}");
        }
        assert_eq!(
            relative_path("db/000001.ldb").unwrap(),
            Path::new("db").join("000001.ldb")
        );
    }
    #[test]
    fn archive_round_trip_and_existing_destination_protection() {
        let root = std::env::temp_dir().join(format!(
            "voxel-archive-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir(&root).unwrap();
        let src = root.join("source");
        fs::create_dir_all(src.join("db")).unwrap();
        fs::write(src.join("level.dat"), b"metadata").unwrap();
        fs::write(src.join("db/CURRENT"), b"MANIFEST-000001\n").unwrap();
        let archive = root.join("test.mcworld");
        pack(&src, &archive).unwrap();
        assert!(pack(&src, &archive).is_err());
        let dst = root.join("restored");
        unpack(&archive, &dst).unwrap();
        assert_eq!(fs::read(dst.join("level.dat")).unwrap(), b"metadata");
        assert!(unpack(&archive, &dst).is_err());
        assert_eq!(
            fs::read(dst.join("db/CURRENT")).unwrap(),
            b"MANIFEST-000001\n"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
