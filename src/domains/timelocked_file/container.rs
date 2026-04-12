//! High-level parser for the Timelocked container format,
//! separating the fixed prefix, cleartext header, and locating the payload data.

use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::Path;

use crate::base::Result;

use super::{parse_header, read_file_prefix, write_file_prefix, TimelockedHeader, FORMAT_VERSION};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadKind {
    Text,
    File { original_filename: String },
}

#[derive(Debug, Clone)]
pub struct ParsedContainer {
    pub prefix: super::FilePrefix,
    pub header: TimelockedHeader,
    pub header_bytes: Vec<u8>,
    pub payload_offset: u64,
}

impl ParsedContainer {
    pub fn payload_kind(&self) -> PayloadKind {
        match self.header.original_filename.as_deref() {
            Some(original_filename) => PayloadKind::File {
                original_filename: original_filename.to_string(),
            },
            None => PayloadKind::Text,
        }
    }
}

pub fn parse_container(path: &Path) -> Result<ParsedContainer> {
    let mut file = File::open(path)?;
    parse_container_reader(&mut file)
}

pub fn parse_container_reader(reader: &mut (impl Read + Seek)) -> Result<ParsedContainer> {
    let prefix = read_file_prefix(reader)?;
    let mut header_bytes = vec![0_u8; prefix.header_len as usize];
    reader.read_exact(&mut header_bytes)?;
    let header = parse_header(&header_bytes)?;
    let payload_offset = reader.stream_position()?;

    Ok(ParsedContainer {
        prefix,
        header,
        header_bytes,
        payload_offset,
    })
}

pub fn write_container(
    writer: &mut impl Write,
    header_bytes: &[u8],
    payload_len: u64,
    payload_reader: &mut impl Read,
) -> Result<u64> {
    write_file_prefix(
        writer,
        FORMAT_VERSION,
        0,
        header_bytes.len() as u32,
        payload_len,
    )?;
    writer.write_all(header_bytes)?;
    let copied = std::io::copy(payload_reader, writer)?;
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::PayloadKind;
    use crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder;

    #[test]
    fn payload_kind_reports_text_payloads_without_original_filename() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("message.timelocked");

        let parsed = SampleTimelockedFileBuilder::new(b"hello")
            .write_and_parse(&path)
            .expect("parse container");

        assert_eq!(parsed.payload_kind(), PayloadKind::Text);
    }

    #[test]
    fn payload_kind_reports_file_payloads_with_original_filename() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("message.timelocked");

        let parsed = SampleTimelockedFileBuilder::new(b"hello")
            .original_filename("note.txt")
            .write_and_parse(&path)
            .expect("parse container");

        assert_eq!(
            parsed.payload_kind(),
            PayloadKind::File {
                original_filename: "note.txt".to_string(),
            }
        );
    }
}
