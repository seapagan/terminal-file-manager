//! Offline checks for this snapshot; no server or filesystem access.

/// Decode and re-encode an opaque binary HANDLE without interpreting UTF-8.
pub fn check_handle_bytes() -> Result<(), crate::Error> {
    let packet = [102, 0, 0, 0, 7, 0, 0, 0, 4, 0, 128, 255, 92];
    let decoded = crate::from_bytes(&packet)?;
    let encoded = crate::to_bytes(decoded)?;
    assert_eq!(&encoded[4..], &packet);
    Ok(())
}

/// Exercise every handle-consuming request through the actual wire serializer.
pub fn check_handle_requests() -> Result<(), crate::Error> {
    use crate::{Packet, requests};
    let handle = b"\0\x80\xff\\".as_slice();
    let attrs = crate::fs::Attrs::default();
    let requests: [Packet; 7] = [
        requests::Close::new(handle).into(),
        requests::Read::new(handle, 0, 1).into(),
        requests::Write::new(handle, 0, b"x".as_slice()).into(),
        requests::Fstat::new(handle).into(),
        requests::FSetStat::new(handle, &attrs).into(),
        requests::ReadDir::new(handle).into(),
        requests::Extended::new("fsync@openssh.com", requests::ExtendedFsync::new(handle)).into(),
    ];
    for request in requests {
        let extended = matches!(request, Packet::ExtendedFsync(_));
        let bytes = crate::to_bytes(request)?;
        let offset = if extended {
            13 + "fsync@openssh.com".len()
        } else {
            9
        };
        assert_eq!(&bytes[offset..offset + 4], &[0, 0, 0, 4]);
        assert_eq!(&bytes[offset + 4..offset + 8], handle);
        if extended {
            assert_eq!(&bytes[13..offset], b"fsync@openssh.com");
        }
    }
    Ok(())
}

/// Raw POSIX identities stay distinct on every client operating system.
pub fn check_path_bytes() -> Result<(), crate::Error> {
    use crate::{
        fs::{Attrs, Flags},
        requests,
    };
    let attrs = Attrs::default();
    for name in [
        b"/raw-\xff\\\n".as_slice(),
        b"/raw-\xfe\\\n",
        "/raw-�\\\n".as_bytes(),
    ] {
        let path = typed_path::UnixPathBuf::from(name);
        let bytes = crate::to_bytes(requests::Open::new(&path, Flags::READ, &attrs))?;
        assert_eq!(
            &bytes[9..13],
            &u32::try_from(name.len()).unwrap().to_be_bytes()
        );
        assert_eq!(&bytes[13..13 + name.len()], name);
        assert_eq!(path.as_bytes(), name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn handle_response_preserves_bytes() {
        super::check_handle_bytes().unwrap();
    }

    #[test]
    fn handle_requests_preserve_bytes() {
        super::check_handle_requests().unwrap();
    }

    #[test]
    fn remote_paths_preserve_bytes() {
        super::check_path_bytes().unwrap();
    }
}
