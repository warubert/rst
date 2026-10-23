use super::is_mp4_subtitle_handler;
use super::mp4_metadata::scan_mp4_hearing_impaired_flags;
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
