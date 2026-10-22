use md5::{Digest as Md5Digest, Md5};
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::io::Read;

#[derive(Serialize)]
pub struct Checksum {
    algorithm: &'static str,
    value: String,
}

/// Read once, with bounded memory, regardless of file size or algorithm count.
pub fn calculate(path: &str) -> Result<Vec<Checksum>, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("请选择文件".into());
    }
    let mut md5 = Md5::new();
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut sha512 = Sha512::new();
    let mut crc = crc32fast::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        let bytes = &buffer[..count];
        md5.update(bytes);
        sha1.update(bytes);
        sha256.update(bytes);
        sha512.update(bytes);
        crc.update(bytes);
    }
    Ok(vec![
        Checksum {
            algorithm: "SHA-256",
            value: format!("{:x}", sha256.finalize()),
        },
        Checksum {
            algorithm: "SHA-512",
            value: format!("{:x}", sha512.finalize()),
        },
        Checksum {
            algorithm: "SHA-1",
            value: format!("{:x}", sha1.finalize()),
        },
        Checksum {
            algorithm: "MD5",
            // md-5 0.11 returns a hybrid_array digest that does not implement
            // LowerHex, so render the bytes explicitly.
            value: md5
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        },
        Checksum {
            algorithm: "CRC32",
            value: format!("{:08x}", crc.finalize()),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_vectors() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"abc").unwrap();
        let hashes = calculate(file.path().to_str().unwrap()).unwrap();
        assert_eq!(
            hashes[0].value,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hashes[2].value, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hashes[3].value, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hashes[4].value, "352441c2");
    }
}
