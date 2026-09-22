use super::{archive, session::WorldStore};
use crate::chunk::Chunk;
use std::{io, path::Path};

fn value<'a>(args: &'a [String], flag: &str) -> io::Result<Option<&'a str>> {
    for (i, arg) in args.iter().enumerate() {
        let result = if arg == flag {
            args.get(i + 1).map(String::as_str)
        } else if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
            Some(value)
        } else {
            continue;
        };
        return result
            .filter(|value| !value.is_empty() && !value.starts_with("--"))
            .map(Some)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{flag} requires a value"),
                )
            });
    }
    Ok(None)
}

/// Headless world utilities run before window or GPU initialization.
pub fn run(args: &[String]) -> io::Result<bool> {
    let export = value(args, "--export-bedrock")?;
    let pack = value(args, "--pack-mcworld")?;
    let unpack = value(args, "--unpack-mcworld")?;
    if [export, pack, unpack]
        .iter()
        .filter(|value| value.is_some())
        .count()
        > 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose one Bedrock world operation",
        ));
    }
    if let Some(path) = export {
        let radius = value(args, "--export-radius")?
            .unwrap_or("1")
            .parse::<i32>()
            .ok()
            .filter(|radius| (0..=16).contains(radius))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Export radius must be 0..16")
            })?;
        let seed = value(args, "--seed")?
            .unwrap_or("0")
            .parse::<i64>()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Seed must be a signed 64-bit integer",
                )
            })? as u64;
        let store = WorldStore::create(Path::new(path), seed)?;
        for x in -radius..=radius {
            for z in -radius..=radius {
                let mut chunk = Chunk::new(x, z, seed);
                chunk.generate();
                store.insert_generated(&chunk, 0)?;
            }
        }
        println!(
            "Wrote {} native Bedrock chunks to {path}",
            (2 * radius + 1).pow(2)
        );
        return Ok(true);
    }
    if let Some(source) = pack.or(unpack) {
        let destination = value(args, "--output")?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Archive operations require --output",
            )
        })?;
        if pack.is_some() {
            // Hold the same lock as gameplay for the entire coherent snapshot.
            let _lock = super::lock::DatabaseLock::acquire(&Path::new(source).join("db"))?;
            archive::pack(Path::new(source), Path::new(destination))?;
        } else {
            archive::unpack(Path::new(source), Path::new(destination))?;
        }
        println!("Created {destination}");
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_flags_require_explicit_paths_and_reject_ambiguous_operations() {
        for args in [
            vec!["--pack-mcworld"],
            vec!["--pack-mcworld", "world"],
            vec!["--export-bedrock="],
            vec!["--export-bedrock", "world", "--export-radius", "-1"],
            vec!["--export-bedrock", "world", "--pack-mcworld", "world"],
        ] {
            assert!(run(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
        assert!(!run(&["--save".into(), "world.vps".into()]).unwrap());
    }
}
