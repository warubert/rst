use rust_i18n::t;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};

#[derive(Debug, Clone, Copy)]
pub(super) struct Mp4Atom {
    pub(super) kind: [u8; 4],
    pub(super) payload_start: u64,
    pub(super) end: u64,
}

pub(super) fn mp4_hearing_impaired_flags(input: &str) -> Result<Vec<bool>, String> {
    let mut file = File::open(input)
        .map_err(|error| t!("errors.media_open", input = input, error = error).to_string())?;
    let file_size = file
        .metadata()
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?
        .len();
    scan_mp4_hearing_impaired_flags(&mut file, file_size)
        .map_err(|error| t!("errors.media_parse", container = "MP4", error = error).to_string())
}

pub(super) fn scan_mp4_hearing_impaired_flags<R: Read + Seek>(
    reader: &mut R,
    file_size: u64,
) -> io::Result<Vec<bool>> {
    let mut hearing_impaired = Vec::new();
    reader.seek(SeekFrom::Start(0))?;

    while let Some(atom) = read_mp4_atom(reader, file_size)? {
        if &atom.kind == b"moov" {
            reader.seek(SeekFrom::Start(atom.payload_start))?;
            while let Some(child) = read_mp4_atom(reader, atom.end)? {
                if &child.kind == b"trak" {
                    hearing_impaired.push(mp4_track_is_hearing_impaired(reader, child)?);
                }
                reader.seek(SeekFrom::Start(child.end))?;
            }
        }
        reader.seek(SeekFrom::Start(atom.end))?;
    }

    Ok(hearing_impaired)
}

fn mp4_track_is_hearing_impaired<R: Read + Seek>(
    reader: &mut R,
    track: Mp4Atom,
) -> io::Result<bool> {
    reader.seek(SeekFrom::Start(track.payload_start))?;
    let mut is_hearing_impaired = false;

    while let Some(child) = read_mp4_atom(reader, track.end)? {
        if &child.kind == b"udta" {
            reader.seek(SeekFrom::Start(child.payload_start))?;
            while let Some(metadata) = read_mp4_atom(reader, child.end)? {
                if &metadata.kind == b"kind" {
                    is_hearing_impaired |= mp4_kind_is_hearing_impaired(reader, metadata)?;
                }
                reader.seek(SeekFrom::Start(metadata.end))?;
            }
        }
        reader.seek(SeekFrom::Start(child.end))?;
    }

    Ok(is_hearing_impaired)
}

fn mp4_kind_is_hearing_impaired<R: Read + Seek>(reader: &mut R, atom: Mp4Atom) -> io::Result<bool> {
    const MAX_KIND_ATOM_SIZE: u64 = 8192;

    let payload_size = atom.end - atom.payload_start;
    if payload_size > MAX_KIND_ATOM_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "kind atom exceeds the maximum supported size",
        ));
    }

    let mut payload = vec![0; payload_size as usize];
    reader.seek(SeekFrom::Start(atom.payload_start))?;
    reader.read_exact(&mut payload)?;

    if payload.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "kind atom is missing its full-box header",
        ));
    }

    let mut fields = payload[4..].split(|byte| *byte == 0);
    let scheme = fields.next().unwrap_or_default();
    let value = fields.next().unwrap_or_default();

    Ok(scheme == b"urn:mpeg:dash:role:2011" && value == b"caption")
}

fn read_mp4_atom<R: Read + Seek>(reader: &mut R, parent_end: u64) -> io::Result<Option<Mp4Atom>> {
    let start = reader.stream_position()?;
    if start == parent_end {
        return Ok(None);
    }
    if start > parent_end || parent_end - start < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid MP4 atom header",
        ));
    }

    let size = u64::from(reader.read_u32_be()?);
    let mut kind = [0; 4];
    reader.read_exact(&mut kind)?;
    let (size, header_size) = match size {
        1 => (reader.read_u64_be()?, 16),
        0 => (parent_end - start, 8),
        size => (size, 8),
    };
    let end = start
        .checked_add(size)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "MP4 atom size overflow"))?;

    if size < header_size || end > parent_end {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MP4 atom extends beyond its parent",
        ));
    }

    Ok(Some(Mp4Atom {
        kind,
        payload_start: start + header_size,
        end,
    }))
}

trait ReadBigEndian {
    fn read_u32_be(&mut self) -> io::Result<u32>;
    fn read_u64_be(&mut self) -> io::Result<u64>;
}

impl<R: Read> ReadBigEndian for R {
    fn read_u32_be(&mut self) -> io::Result<u32> {
        let mut bytes = [0; 4];
        self.read_exact(&mut bytes)?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn read_u64_be(&mut self) -> io::Result<u64> {
        let mut bytes = [0; 8];
        self.read_exact(&mut bytes)?;
        Ok(u64::from_be_bytes(bytes))
    }
}
