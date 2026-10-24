use super::{
    Options,
    safety::{self, Result},
    staging,
};
use crate::fs::Entry;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    sync::atomic::AtomicBool,
};

mod sparse;

pub(super) fn copy(
    source: &Entry,
    destination: &mut File,
    options: Options,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<u64> {
    let mut input = opened(source)?;
    let metadata = input.metadata()?;
    let allocation = sparse::prepare(&input, destination, &metadata)?;
    let (bytes, digest) =
        file_payload(&mut input, destination, options.checksum, cancel, progress)?;
    safety::revalidate(source, true)?;
    metadata_verification(destination, &metadata, bytes)?;
    finish_metadata(destination, &metadata, digest, cancel)?;
    sparse::verify(allocation, metadata.len(), destination)?;
    Ok(bytes)
}

fn file_payload(
    input: &mut File,
    destination: &mut File,
    checksum: bool,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<(u64, Option<[u8; 32]>)> {
    let mut output = sparse::Writer::new(destination);
    let copied = payload(input, &mut output, checksum, cancel, progress)?;
    output.finish(copied.0)?;
    Ok(copied)
}

fn finish_metadata(
    destination: &mut File,
    metadata: &Metadata,
    digest: Option<[u8; 32]>,
    cancel: &AtomicBool,
) -> Result<()> {
    if let Some(digest) = digest {
        checksum_verification(destination, digest, cancel)?;
    }
    destination.set_times(
        fs::FileTimes::new()
            .set_modified(metadata.modified()?)
            .set_accessed(metadata.accessed()?),
    )?;
    destination.sync_all()?;
    Ok(())
}

fn opened(source: &Entry) -> Result<File> {
    let file = staging::open_source(&source.path)?;
    safety::opened_object(source, &file)?;
    safety::revalidate(source, true)?;
    let metadata = file.metadata()?;
    if metadata.len() != source.size.unwrap_or(u64::MAX)
        || metadata.modified().ok() != source.modified
    {
        return Err(safety::changed("Opened source differs from approved entry"));
    }
    Ok(file)
}

fn payload(
    input: &mut impl Read,
    output: &mut impl Write,
    checksum: bool,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<(u64, Option<[u8; 32]>)> {
    let mut digest = checksum.then(Sha256::new);
    let mut buffer = vec![0; 64 * 1024];
    let mut bytes = 0u64;
    loop {
        crate::fs::check_cancelled(cancel)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        if let Some(digest) = &mut digest {
            digest.update(&buffer[..count]);
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or_else(|| super::plan::invalid("Byte count overflow"))?;
        progress(bytes);
    }
    Ok((bytes, digest.map(|digest| digest.finalize().into())))
}

fn metadata_verification(destination: &mut File, source: &Metadata, bytes: u64) -> Result<()> {
    if bytes != source.len() {
        return Err(safety::changed("Source byte count changed during read"));
    }
    destination.set_permissions(source.permissions())?;
    destination.flush()?;
    if destination.metadata()?.len() != bytes {
        return Err(safety::changed("Staged destination size mismatch"));
    }
    Ok(())
}

fn checksum_verification(
    destination: &mut File,
    expected: [u8; 32],
    cancel: &AtomicBool,
) -> Result<()> {
    destination.seek(SeekFrom::Start(0))?;
    let (_, digest) = payload(destination, &mut std::io::sink(), true, cancel, &mut |_| {})?;
    if digest != Some(expected) {
        return Err(safety::changed("SHA-256 verification failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    struct ShortWriter {
        bytes: Vec<u8>,
        fail_after: usize,
    }

    impl Write for ShortWriter {
        fn write(&mut self, input: &[u8]) -> io::Result<usize> {
            if self.bytes.len() >= self.fail_after {
                return Err(io::Error::from_raw_os_error(28));
            }
            let count = input.len().min(3).min(self.fail_after - self.bytes.len());
            self.bytes.extend_from_slice(&input[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn short_writes_complete_and_enospc_never_produces_verified_bytes() {
        let bytes = b"a payload exceeding one short write";
        let mut output = ShortWriter {
            bytes: Vec::new(),
            fail_after: usize::MAX,
        };
        let copied = payload(
            &mut bytes.as_slice(),
            &mut output,
            true,
            &false.into(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(output.bytes, bytes);
        assert_eq!(copied.0, 35);
        assert!(copied.1.is_some());
        let mut output = ShortWriter {
            bytes: Vec::new(),
            fail_after: 7,
        };
        let error = payload(
            &mut bytes.as_slice(),
            &mut output,
            false,
            &false.into(),
            &mut |_| {},
        )
        .err()
        .unwrap();
        assert_eq!(error.outcome, super::super::Outcome::Failed);
        assert_eq!(output.bytes, b"a paylo");
    }
}
