//! Persistent Bedrock subchunks. Preserve all palette states and storage layers.
use super::nbt;
use crate::java_compat::NbtTag;
use std::io;

const CELLS: usize = 4096;
const WIDTHS: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 8, 16];

#[derive(Clone, Debug, PartialEq)]
pub struct Storage {
    pub palette: Vec<NbtTag>,
    /// X-major, Z-middle, Y-minor order (different from the engine's terrain).
    pub indices: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subchunk {
    pub y: i8,
    pub layers: Vec<Storage>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn take<'a>(bytes: &mut &'a [u8], len: usize) -> io::Result<&'a [u8]> {
    if bytes.len() < len {
        return Err(invalid("Truncated subchunk"));
    }
    let (value, tail) = bytes.split_at(len);
    *bytes = tail;
    Ok(value)
}
fn byte(bytes: &mut &[u8]) -> io::Result<u8> {
    Ok(take(bytes, 1)?[0])
}
fn u32_le(bytes: &mut &[u8]) -> io::Result<u32> {
    Ok(u32::from_le_bytes(take(bytes, 4)?.try_into().unwrap()))
}

impl Subchunk {
    pub fn decode(mut bytes: &[u8], section_y: i8) -> io::Result<Self> {
        let version = byte(&mut bytes)?;
        let (layers, y) = match version {
            1 => (1, section_y),
            8 => (byte(&mut bytes)?, section_y),
            9 => {
                let layers = byte(&mut bytes)?;
                (layers, byte(&mut bytes)? as i8)
            }
            _ => return Err(invalid("Unsupported Bedrock subchunk version")),
        };
        if y != section_y || layers > 16 {
            return Err(invalid("Invalid subchunk location or layer count"));
        }
        let mut result = Self {
            y,
            layers: Vec::new(),
        };
        for _ in 0..layers {
            let header = byte(&mut bytes)?;
            let width = header >> 1;
            if header & 1 != 0 || !WIDTHS.contains(&width) {
                return Err(invalid("Expected a persistent block palette"));
            }
            let mut indices = vec![0u16; CELLS];
            if width != 0 {
                let per_word = 32 / width as usize;
                let mask = (1u32 << width) - 1;
                for cells in indices.chunks_mut(per_word) {
                    let word = u32_le(&mut bytes)?;
                    for (i, cell) in cells.iter_mut().enumerate() {
                        *cell = ((word >> (i * width as usize)) & mask) as u16;
                    }
                }
            }
            let count = if width == 0 {
                1
            } else {
                u32_le(&mut bytes)? as usize
            };
            if count == 0 || count > CELLS || (width != 0 && count > 1usize << width) {
                return Err(invalid("Invalid subchunk palette size"));
            }
            let mut palette = Vec::new();
            for _ in 0..count {
                let (_, state, consumed) = nbt::decode(bytes)?;
                take(&mut bytes, consumed)?;
                if !matches!(state.get("name"), Some(NbtTag::String(_)))
                    || !matches!(state.get("states"), Some(NbtTag::Compound(_)))
                {
                    return Err(invalid("Malformed block-state palette entry"));
                }
                palette.push(state);
            }
            if indices.iter().any(|i| *i as usize >= count) {
                return Err(invalid("Subchunk palette index is out of bounds"));
            }
            result.layers.push(Storage { palette, indices });
        }
        if !bytes.is_empty() {
            return Err(invalid("Trailing subchunk data"));
        }
        Ok(result)
    }

    pub fn encode(&self) -> io::Result<Vec<u8>> {
        if self.layers.len() > 16 {
            return Err(invalid("Too many subchunk layers"));
        }
        let mut out = vec![9, self.layers.len() as u8, self.y as u8];
        for layer in &self.layers {
            let count = layer.palette.len();
            if count == 0
                || count > CELLS
                || layer.indices.len() != CELLS
                || layer.indices.iter().any(|i| *i as usize >= count)
            {
                return Err(invalid("Invalid subchunk palette"));
            }
            let width = *WIDTHS.iter().find(|&&w| count <= 1usize << w).unwrap();
            out.push(width << 1);
            if width != 0 {
                for cells in layer.indices.chunks(32 / width as usize) {
                    let mut word = 0u32;
                    for (i, &cell) in cells.iter().enumerate() {
                        word |= (cell as u32) << (i * width as usize);
                    }
                    out.extend_from_slice(&word.to_le_bytes());
                }
                out.extend_from_slice(&(count as u32).to_le_bytes());
            }
            for state in &layer.palette {
                out.extend_from_slice(&nbt::encode("", state)?);
            }
        }
        Ok(out)
    }
}

pub fn chunk_key(x: i32, z: i32, dimension: i32, tag: u8, section: Option<i8>) -> Vec<u8> {
    let mut key = Vec::with_capacity(14);
    key.extend_from_slice(&x.to_le_bytes());
    key.extend_from_slice(&z.to_le_bytes());
    if dimension != 0 {
        key.extend_from_slice(&dimension.to_le_bytes());
    }
    key.push(tag);
    if let Some(y) = section {
        key.push(y as u8);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(i: usize) -> NbtTag {
        NbtTag::Compound(vec![
            ("name".into(), NbtTag::String(format!("example:block_{i}"))),
            (
                "states".into(),
                NbtTag::Compound(vec![("orientation".into(), NbtTag::Int(i as i32))]),
            ),
            ("version".into(), NbtTag::Int(18168865)),
        ])
    }
    #[test]
    fn all_packing_widths_and_unknown_states_round_trip() {
        for count in [1, 2, 4, 8, 16, 32, 64, 256, 257] {
            let layer = Storage {
                palette: (0..count).map(state).collect(),
                indices: (0..CELLS).map(|i| (i % count) as u16).collect(),
            };
            let subchunk = Subchunk {
                y: -4,
                layers: vec![layer.clone(), layer],
            };
            let encoded = subchunk.encode().unwrap();
            assert_eq!(Subchunk::decode(&encoded, -4).unwrap(), subchunk);
            assert!(Subchunk::decode(&encoded, 0).is_err());
            assert!(Subchunk::decode(&encoded[..encoded.len() - 1], -4).is_err());
        }
    }
    #[test]
    fn keys_separate_dimensions_and_negative_coordinates() {
        let a = chunk_key(-1, -2, 0, 0x2f, Some(-4));
        assert_eq!(&a[..4], &[255; 4]);
        assert_eq!(a.len(), 10);
        let b = chunk_key(-1, -2, 1, 0x2f, Some(-4));
        assert_eq!(&b[8..12], &[1, 0, 0, 0]);
        assert_eq!(b.len(), 14);
        assert_ne!(a, b);
    }
    #[test]
    fn corrupt_or_network_palettes_are_rejected() {
        assert!(Subchunk::decode(&[9, 1, 0, 1], 0).is_err());
        assert!(Subchunk::decode(&[10, 0, 0], 0).is_err());
        let invalid_layer = Storage {
            palette: vec![state(0)],
            indices: vec![1; CELLS],
        };
        assert!(
            Subchunk {
                y: 0,
                layers: vec![invalid_layer]
            }
            .encode()
            .is_err()
        );
    }
}
