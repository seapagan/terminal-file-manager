use crate::operations::safety::{self, Result};
use std::{
    fs::{File, Metadata},
    io::{self, Seek, SeekFrom, Write},
};

#[cfg(windows)]
mod windows;

// Coalesce zero sectors into holes; logical bytes still pass through SHA-256.
pub(super) struct Writer<'a> {
    file: &'a mut File,
    position: u64,
    materialized: u64,
}

impl<'a> Writer<'a> {
    pub fn new(file: &'a mut File) -> Self {
        Self {
            file,
            position: 0,
            materialized: 0,
        }
    }
    pub fn finish(self, bytes: u64) -> Result<()> {
        if self.file.metadata()?.len() != self.materialized {
            return Err(safety::changed("Staged destination size mismatch"));
        }
        if self.materialized < bytes {
            self.file.set_len(bytes)?;
        }
        Ok(())
    }

    fn region(&mut self, bytes: &[u8], zero: bool) -> io::Result<()> {
        let end = self.end(bytes.len())?;
        if zero {
            self.file.seek(SeekFrom::Start(end))?;
        } else {
            self.file.write_all(bytes)?;
            self.materialized = end;
        }
        self.position = end;
        Ok(())
    }

    fn end(&self, count: usize) -> io::Result<u64> {
        self.position
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("Sparse byte count overflow"))
    }
}

impl Write for Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.end(bytes.len())?;
        let base = self.position;
        let (mut start, mut offset, mut zero) = (0, 0, false);
        while offset < bytes.len() {
            let count = (512 - (base + offset as u64) % 512) as usize;
            let end = (offset + count).min(bytes.len());
            let is_zero = bytes[offset..end].iter().all(|byte| *byte == 0);
            if is_zero != zero {
                if start < offset {
                    self.region(&bytes[start..offset], zero)?;
                }
                start = offset;
                zero = is_zero;
            }
            offset = end;
        }
        if start < bytes.len() {
            self.region(&bytes[start..], zero)?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(unix)]
pub(super) fn prepare(_source_file: &File, _target: &File, source: &Metadata) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(source.blocks().saturating_mul(512))
}

#[cfg(windows)]
pub(super) fn prepare(source_file: &File, target: &File, source: &Metadata) -> io::Result<u64> {
    use std::os::windows::fs::MetadataExt;
    if source.file_attributes() & 0x200 != 0 {
        windows::mark_sparse(target)?;
        windows::allocated(source_file)
    } else {
        Ok(source.len())
    }
}

#[cfg(unix)]
fn allocated(file: &File) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(file.metadata()?.blocks().saturating_mul(512))
}

#[cfg(windows)]
use windows::allocated;

pub(super) fn verify(source_allocation: u64, size: u64, target: &File) -> io::Result<()> {
    if source_allocation < size / 2 && allocated(target)? >= size / 2 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Destination allocation expanded; sparse/compressed fidelity unsupported on this filesystem; transfer refused",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn native_allocation(expected: &[u8]) -> u64 {
        let mut file = tempfile::tempfile().unwrap();
        #[cfg(windows)]
        windows::mark_sparse(&file).unwrap();
        file.write_all(&expected[..1024]).unwrap();
        file.seek(SeekFrom::Start(512 * 1024)).unwrap();
        file.write_all(&expected[512 * 1024..512 * 1024 + 512])
            .unwrap();
        file.set_len(expected.len() as u64).unwrap();
        file.sync_all().unwrap();
        allocated(&file).unwrap()
    }

    #[test]
    fn unaligned_short_writes_preserve_zero_runs_trailing_length_and_all_bytes() {
        let mut output = tempfile::tempfile().unwrap();
        #[cfg(windows)]
        windows::mark_sparse(&output).unwrap();
        let mut expected = vec![0; 1024 * 1024];
        expected[..1024].fill(1);
        expected[512 * 1024..512 * 1024 + 512].fill(2);
        let mut writer = Writer::new(&mut output);
        for part in expected.chunks(511) {
            writer.write_all(part).unwrap();
        }
        writer.finish(expected.len() as u64).unwrap();
        output.sync_all().unwrap();
        let allocation = allocated(&output).unwrap();
        let native = native_allocation(&expected);
        assert!(
            allocation <= native,
            "{allocation} allocated versus {native} native of {} logical bytes; temp directory {:?}",
            expected.len(),
            std::env::temp_dir()
        );
        output.seek(SeekFrom::Start(0)).unwrap();
        let mut actual = Vec::new();
        output.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }
}
