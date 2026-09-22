//! Little-endian, on-disk NBT (not network/varint NBT).
use crate::java_compat::NbtTag;
use std::io;

const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_NODES: usize = 1_000_000;
const MAX_DEPTH: usize = 64;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn room(out: &[u8], additional: usize) -> io::Result<()> {
    if additional > MAX_BYTES.saturating_sub(out.len()) {
        return Err(invalid("NBT size limit exceeded"));
    }
    Ok(())
}
fn text(out: &mut Vec<u8>, value: &str) -> io::Result<()> {
    let len = u16::try_from(value.len()).map_err(|_| invalid("NBT string is too long"))?;
    room(out, 2 + value.len())?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn count(out: &mut Vec<u8>, len: usize) -> io::Result<()> {
    let len = i32::try_from(len).map_err(|_| invalid("NBT collection is too long"))?;
    out.extend_from_slice(&len.to_le_bytes());
    Ok(())
}
fn payload(out: &mut Vec<u8>, tag: &NbtTag, depth: usize, nodes: &mut usize) -> io::Result<()> {
    if depth > MAX_DEPTH || *nodes >= MAX_NODES || out.len() > MAX_BYTES {
        return Err(invalid("NBT size or nesting limit exceeded"));
    }
    *nodes += 1;
    match tag {
        NbtTag::Byte(n) => out.push(*n as u8),
        NbtTag::Short(n) => out.extend_from_slice(&n.to_le_bytes()),
        NbtTag::Int(n) => out.extend_from_slice(&n.to_le_bytes()),
        NbtTag::Long(n) => out.extend_from_slice(&n.to_le_bytes()),
        NbtTag::Float(n) => out.extend_from_slice(&n.to_le_bytes()),
        NbtTag::Double(n) => out.extend_from_slice(&n.to_le_bytes()),
        NbtTag::String(s) => text(out, s)?,
        NbtTag::ByteArray(values) => {
            room(out, values.len().saturating_add(4))?;
            count(out, values.len())?;
            out.extend_from_slice(values);
        }
        NbtTag::IntArray(values) => {
            room(out, values.len().saturating_mul(4).saturating_add(4))?;
            count(out, values.len())?;
            for value in values {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        NbtTag::LongArray(values) => {
            room(out, values.len().saturating_mul(8).saturating_add(4))?;
            count(out, values.len())?;
            for value in values {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        NbtTag::List { element_type, tags } => {
            if *element_type > 12 || tags.iter().any(|t| t.id() != *element_type) {
                return Err(invalid("NBT list has inconsistent element types"));
            }
            out.push(*element_type);
            count(out, tags.len())?;
            for tag in tags {
                payload(out, tag, depth + 1, nodes)?;
            }
        }
        NbtTag::Compound(fields) => {
            let mut names = std::collections::HashSet::new();
            for (name, tag) in fields {
                if !names.insert(name) {
                    return Err(invalid("Duplicate NBT field"));
                }
                out.push(tag.id());
                text(out, name)?;
                payload(out, tag, depth + 1, nodes)?;
            }
            out.push(0);
        }
    }
    if out.len() > MAX_BYTES {
        return Err(invalid("NBT size limit exceeded"));
    }
    Ok(())
}
pub fn encode(name: &str, tag: &NbtTag) -> io::Result<Vec<u8>> {
    let mut out = vec![tag.id()];
    text(&mut out, name)?;
    payload(&mut out, tag, 0, &mut 0)?;
    Ok(out)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    nodes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> io::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| invalid("NBT size overflow"))?;
        if end > MAX_BYTES {
            return Err(invalid("NBT size limit exceeded"));
        }
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("Truncated NBT"))?;
        self.pos = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn text(&mut self) -> io::Result<String> {
        let len = u16::from_le_bytes(self.array()?) as usize;
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| invalid("Invalid NBT UTF-8"))
    }
    fn count(&mut self, minimum_size: usize) -> io::Result<usize> {
        let len = usize::try_from(i32::from_le_bytes(self.array()?))
            .map_err(|_| invalid("Negative NBT collection size"))?;
        if len > MAX_BYTES / minimum_size.max(1)
            || len.saturating_mul(minimum_size.max(1)) > self.bytes.len() - self.pos
        {
            return Err(invalid("NBT collection exceeds remaining input"));
        }
        Ok(len)
    }
    fn payload(&mut self, id: u8, depth: usize) -> io::Result<NbtTag> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            return Err(invalid("NBT size or nesting limit exceeded"));
        }
        self.nodes += 1;
        Ok(match id {
            1 => NbtTag::Byte(self.byte()? as i8),
            2 => NbtTag::Short(i16::from_le_bytes(self.array()?)),
            3 => NbtTag::Int(i32::from_le_bytes(self.array()?)),
            4 => NbtTag::Long(i64::from_le_bytes(self.array()?)),
            5 => NbtTag::Float(f32::from_le_bytes(self.array()?)),
            6 => NbtTag::Double(f64::from_le_bytes(self.array()?)),
            7 => {
                let len = self.count(1)?;
                NbtTag::ByteArray(self.take(len)?.to_vec())
            }
            8 => NbtTag::String(self.text()?),
            9 => {
                let element_type = self.byte()?;
                let len = self.count(1)?;
                if element_type > 12
                    || (element_type == 0 && len != 0)
                    || len > MAX_NODES - self.nodes
                {
                    return Err(invalid("Invalid NBT list"));
                }
                let mut tags = Vec::new();
                for _ in 0..len {
                    tags.push(self.payload(element_type, depth + 1)?);
                }
                NbtTag::List { element_type, tags }
            }
            10 => {
                let mut fields = Vec::new();
                let mut names = std::collections::HashSet::new();
                loop {
                    let id = self.byte()?;
                    if id == 0 {
                        break;
                    }
                    let name = self.text()?;
                    if !names.insert(name.clone()) {
                        return Err(invalid("Duplicate NBT field"));
                    }
                    fields.push((name, self.payload(id, depth + 1)?));
                }
                NbtTag::Compound(fields)
            }
            11 => {
                let len = self.count(4)?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(i32::from_le_bytes(self.array()?));
                }
                NbtTag::IntArray(values)
            }
            12 => {
                let len = self.count(8)?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(i64::from_le_bytes(self.array()?));
                }
                NbtTag::LongArray(values)
            }
            _ => return Err(invalid("Unknown NBT tag")),
        })
    }
}
pub fn decode(bytes: &[u8]) -> io::Result<(String, NbtTag, usize)> {
    let mut reader = Reader {
        bytes,
        pos: 0,
        nodes: 0,
    };
    let id = reader.byte()?;
    let name = reader.text()?;
    let tag = reader.payload(id, 0)?;
    Ok((name, tag, reader.pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoder_rejects_oversized_arrays_before_copying() {
        let tag = NbtTag::ByteArray(vec![0; MAX_BYTES]);
        let mut out = vec![10, 0, 0];
        assert!(payload(&mut out, &tag, 0, &mut 0).is_err());
        assert_eq!(out.len(), 3);
    }
    #[test]
    fn little_endian_root_matches_wire_bytes() {
        let bytes = [10, 0, 0, 3, 1, 0, b'x', 0x78, 0x56, 0x34, 0x12, 0];
        let root = NbtTag::Compound(vec![("x".into(), NbtTag::Int(0x12345678))]);
        assert_eq!(encode("", &root).unwrap(), bytes);
        let (name, decoded, used) = decode(&bytes).unwrap();
        assert_eq!(name, "");
        assert_eq!(used, bytes.len());
        assert_eq!(encode("", &decoded).unwrap(), bytes);
    }
    #[test]
    fn all_disk_tag_types_round_trip() {
        let root = NbtTag::Compound(vec![
            ("byte".into(), NbtTag::Byte(-2)),
            ("short".into(), NbtTag::Short(-513)),
            ("long".into(), NbtTag::Long(i64::MIN)),
            ("float".into(), NbtTag::Float(1.5)),
            ("double".into(), NbtTag::Double(-2.25)),
            ("text".into(), NbtTag::String("forêt".into())),
            ("bytes".into(), NbtTag::ByteArray(vec![0, 128, 255])),
            ("ints".into(), NbtTag::IntArray(vec![-1, 0, i32::MAX])),
            ("longs".into(), NbtTag::LongArray(vec![i64::MIN, i64::MAX])),
            (
                "list".into(),
                NbtTag::List {
                    element_type: 10,
                    tags: vec![NbtTag::Compound(vec![])],
                },
            ),
        ]);
        let bytes = encode("world", &root).unwrap();
        let (name, decoded, used) = decode(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(encode(&name, &decoded).unwrap(), bytes);
        for len in 0..bytes.len() {
            assert!(decode(&bytes[..len]).is_err());
        }
    }
    #[test]
    fn reject_malicious_lengths_and_nesting_without_allocating_them() {
        assert!(decode(&[7, 0, 0, 255, 255, 255, 127]).is_err());
        assert!(decode(&[9, 0, 0, 0, 255, 255, 255, 127]).is_err());
        let mut nested = vec![];
        for _ in 0..100 {
            nested.extend_from_slice(&[10, 0, 0]);
        }
        nested.extend_from_slice(&[0; 100]);
        assert!(decode(&nested).is_err());
        assert!(
            encode(
                "",
                &NbtTag::List {
                    element_type: 3,
                    tags: vec![NbtTag::Byte(0)]
                }
            )
            .is_err()
        );
    }
}
