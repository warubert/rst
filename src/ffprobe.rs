use crate::interfaces::probe::{Subtitle, SubtitleProbe};
use rust_i18n::t;
use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

pub struct Ffprobe;

impl SubtitleProbe for Ffprobe {
    fn get_subtitle_streams(&self, input: &str) -> Result<Vec<Subtitle>, String> {
        get_subtitle_streams(input)
    }
}

fn get_subtitle_streams(input: &str) -> Result<Vec<Subtitle>, String> {
    let mut file = File::open(input)
        .map_err(|error| t!("errors.media_open", input = input, error = error).to_string())?;
    let mut signature = [0; 8];
    let signature_len = file
        .read(&mut signature)
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?;

    let extension = Path::new(input)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if (signature_len >= 4 && signature[..4] == [0x1a, 0x45, 0xdf, 0xa3])
        || matches!(extension.as_str(), "mkv" | "webm")
    {
        get_matroska_subtitle_streams(file)
    } else if (signature_len >= 8 && &signature[4..8] == b"ftyp")
        || matches!(extension.as_str(), "mp4" | "mov")
    {
        get_mp4_subtitle_streams(file, input)
    } else {
        Err(t!("errors.unsupported_media", input = input).to_string())
    }
}

fn get_matroska_subtitle_streams(file: File) -> Result<Vec<Subtitle>, String> {
    let container = matroska::Matroska::open(file).map_err(|error| {
        t!("errors.media_parse", container = "Matroska", error = error).to_string()
    })?;

    Ok(container
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.is_subtitle())
        .map(|(index, track)| Subtitle {
            index: i64::try_from(index).unwrap_or(-1),
            language: track
                .language
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| t!("tui.unknown_language").to_string()),
            is_sdh: track.hearing_impaired.unwrap_or(false),
        })
        .collect())
}

fn get_mp4_subtitle_streams(file: File, input: &str) -> Result<Vec<Subtitle>, String> {
    let file_size = file
        .metadata()
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?
        .len();
    let container = mp4::Mp4Reader::read_header(BufReader::new(file), file_size)
        .map_err(|error| t!("errors.media_parse", container = "MP4", error = error).to_string())?;
    let hearing_impaired_flags = mp4_hearing_impaired_flags(input)?;

    if hearing_impaired_flags.len() != container.moov.traks.len() {
        return Err(t!(
            "errors.media_parse",
            container = "MP4",
            error = "track metadata count does not match"
        )
        .to_string());
    }

    Ok(container
        .moov
        .traks
        .iter()
        .zip(hearing_impaired_flags)
        .enumerate()
        .filter(|(_, (track, _))| {
            is_mp4_subtitle_handler(&track.mdia.hdlr.handler_type.to_string())
        })
        .map(|(index, (track, is_sdh))| Subtitle {
            index: i64::try_from(index).unwrap_or(-1),
            language: track.mdia.mdhd.language.clone(),
            is_sdh,
        })
        .collect())
}

#[derive(Debug, Clone, Copy)]
struct Mp4Atom {
    kind: [u8; 4],
    payload_start: u64,
    end: u64,
}

fn mp4_hearing_impaired_flags(input: &str) -> Result<Vec<bool>, String> {
    let mut file = File::open(input)
        .map_err(|error| t!("errors.media_open", input = input, error = error).to_string())?;
    let file_size = file
        .metadata()
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?
        .len();
    scan_mp4_hearing_impaired_flags(&mut file, file_size)
        .map_err(|error| t!("errors.media_parse", container = "MP4", error = error).to_string())
}

fn scan_mp4_hearing_impaired_flags<R: Read + Seek>(
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

fn is_mp4_subtitle_handler(handler: &str) -> bool {
    matches!(handler, "sbtl" | "subt" | "text" | "clcp")
}

#[cfg(test)]
mod tests {
    use super::{is_mp4_subtitle_handler, scan_mp4_hearing_impaired_flags};
    use std::io::Cursor;

    #[test]
    fn recognizes_common_mp4_subtitle_handlers() {
        for handler in ["sbtl", "subt", "text", "clcp"] {
            assert!(is_mp4_subtitle_handler(handler));
        }
        for handler in ["vide", "soun", "meta"] {
            assert!(!is_mp4_subtitle_handler(handler));
        }
    }

    #[test]
    fn reads_mp4_caption_roles_as_hearing_impaired() {
        fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&u32::try_from(payload.len() + 8).unwrap().to_be_bytes());
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(payload);
            bytes
        }

        let mut kind_payload = vec![0; 4];
        kind_payload.extend_from_slice(b"urn:mpeg:dash:role:2011\0caption\0");
        let kind = atom(b"kind", &kind_payload);
        let udta = atom(b"udta", &kind);
        let track = atom(b"trak", &udta);
        let movie = atom(b"moov", &track);
        let mut reader = Cursor::new(movie.clone());

        assert_eq!(
            scan_mp4_hearing_impaired_flags(&mut reader, movie.len() as u64).unwrap(),
            [true]
        );
    }
}
