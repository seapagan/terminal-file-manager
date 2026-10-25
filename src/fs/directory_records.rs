//! Bounds-checked decoding of Win32 both-ID directory records.
use std::io;

#[derive(Debug, PartialEq)]
pub(super) struct Record {
    pub name: Vec<u16>,
    pub attributes: u32,
    pub length: u64,
    pub created: i64,
    pub accessed: i64,
    pub modified: i64,
    pub id: u64,
}

pub(super) fn parse(buffer: &[u8]) -> io::Result<Vec<Record>> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    loop {
        let remaining = buffer.get(offset..).ok_or_else(invalid)?;
        let next = u32::from_le_bytes(field(remaining, 0)?) as usize;
        let row = extent(remaining, next)?;
        let record = decode(row)?;
        if record.name != [46] && record.name != [46, 46] {
            records.push(record);
        }
        if next == 0 {
            return Ok(records);
        }
        offset = offset.checked_add(next).ok_or_else(invalid)?;
    }
}

fn decode(row: &[u8]) -> io::Result<Record> {
    let name = name(row)?;
    Ok(Record {
        name,
        attributes: u32::from_le_bytes(field(row, 56)?),
        length: u64::from_le_bytes(field(row, 40)?),
        created: i64::from_le_bytes(field(row, 8)?),
        accessed: i64::from_le_bytes(field(row, 16)?),
        modified: i64::from_le_bytes(field(row, 24)?),
        id: u64::from_le_bytes(field(row, 96)?),
    })
}

fn extent(remaining: &[u8], next: usize) -> io::Result<&[u8]> {
    if next == 0 {
        return Ok(remaining);
    }
    if !next.is_multiple_of(8) || next < 104 {
        return Err(invalid());
    }
    remaining.get(..next).ok_or_else(invalid)
}

fn name(row: &[u8]) -> io::Result<Vec<u16>> {
    let length = u32::from_le_bytes(field(row, 60)?) as usize;
    if length == 0 || !length.is_multiple_of(2) {
        return Err(invalid());
    }
    let end = 104usize.checked_add(length).ok_or_else(invalid)?;
    let name: Vec<_> = row
        .get(104..end)
        .ok_or_else(invalid)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    if name.iter().any(|c| matches!(*c, 0 | 47 | 58 | 92)) {
        return Err(invalid());
    }
    Ok(name)
}

fn field<const N: usize>(row: &[u8], start: usize) -> io::Result<[u8; N]> {
    row.get(start..start.checked_add(N).ok_or_else(invalid)?)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(invalid)
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid Win32 directory record")
}

pub(super) fn volume_root(path: &[u16]) -> Option<Vec<u16>> {
    let prefix: Vec<_> = "\\\\?\\Volume{".encode_utf16().collect();
    if !path.starts_with(&prefix) || path.len() < 49 {
        return None;
    }
    for (index, unit) in path[11..47].iter().enumerate() {
        let valid = if matches!(index, 8 | 13 | 18 | 23) {
            *unit == 45
        } else {
            matches!(*unit, 48..=57 | 65..=70 | 97..=102)
        };
        if !valid {
            return None;
        }
    }
    (path[47..49] == [125, 92]).then(|| path[..49].iter().copied().chain([0]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_native_volume_guid_roots_can_identify_local_storage() {
        let path = "\\\\?\\Volume{12345678-1234-5678-9abc-123456789abc}\\folder";
        let root = volume_root(&path.encode_utf16().collect::<Vec<_>>()).unwrap();
        assert_eq!(
            root,
            "\\\\?\\Volume{12345678-1234-5678-9abc-123456789abc}\\\0"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        for invalid in [
            "C:\\folder",
            "\\\\server\\share",
            "\\\\?\\UNC\\server\\share",
            "\\\\?\\Volume{12345678-1234-5678-9abc-123456789abg}\\",
            "\\\\?\\Volume{12345678-1234-5678-9abc-123456789abc}/",
        ] {
            assert!(volume_root(&invalid.encode_utf16().collect::<Vec<_>>()).is_none());
        }
    }

    fn record(name: &[u16]) -> Vec<u8> {
        let mut bytes = vec![0; 104 + name.len() * 2];
        bytes[8..16].copy_from_slice(&123i64.to_le_bytes());
        bytes[16..24].copy_from_slice(&456i64.to_le_bytes());
        bytes[24..32].copy_from_slice(&789i64.to_le_bytes());
        bytes[40..48].copy_from_slice(&42u64.to_le_bytes());
        bytes[56..60].copy_from_slice(&0x22u32.to_le_bytes());
        bytes[60..64].copy_from_slice(&(name.len() as u32 * 2).to_le_bytes());
        bytes[96..104].copy_from_slice(&u64::MAX.to_le_bytes());
        for (pair, unit) in bytes[104..].as_chunks_mut::<2>().0.iter_mut().zip(name) {
            pair.copy_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn preserves_native_name_and_complete_unsigned_identity() {
        let parsed = parse(&record(&[0xd800, 0x65, 0x301, 0x20, 0x2e])).unwrap();
        assert_eq!(
            parsed,
            vec![Record {
                name: vec![0xd800, 0x65, 0x301, 0x20, 0x2e],
                attributes: 0x22,
                length: 42,
                created: 123,
                accessed: 456,
                modified: 789,
                id: u64::MAX,
            }]
        );
    }

    #[test]
    fn bounds_each_name_to_its_record_and_rejects_bad_chains() {
        let valid = record(&[65]);
        for end in 0..valid.len() {
            assert!(parse(&valid[..end]).is_err(), "truncation {end}");
        }
        for offset in [1u32, 104, 108, 112, u32::MAX] {
            let mut bytes = valid.clone();
            bytes[..4].copy_from_slice(&offset.to_le_bytes());
            assert!(parse(&bytes).is_err(), "offset {offset}");
        }
        for length in [0u32, 1, 4, u32::MAX] {
            let mut bytes = valid.clone();
            bytes[60..64].copy_from_slice(&length.to_le_bytes());
            assert!(parse(&bytes).is_err(), "name length {length}");
        }
    }

    #[test]
    fn reads_multiple_records_but_never_treats_padding_as_records() {
        let mut bytes = record(&[65]);
        bytes.resize(112, 0);
        bytes[..4].copy_from_slice(&112u32.to_le_bytes());
        bytes.extend(record(&[66]));
        bytes.resize(65536, 0);
        let rows = parse(&bytes).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, [65]);
        assert_eq!(rows[1].name, [66]);
    }

    #[test]
    fn rejects_operands_that_escape_the_enumerated_directory() {
        for name in [vec![0], vec![65, 92, 66], vec![47], vec![58]] {
            assert!(parse(&record(&name)).is_err());
        }
        assert!(parse(&record(&[46])).unwrap().is_empty());
        assert!(parse(&record(&[46, 46])).unwrap().is_empty());
    }
}
