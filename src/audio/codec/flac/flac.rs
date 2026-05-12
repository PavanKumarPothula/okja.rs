mod flac {
    use crate::audio::codec::flac::dr_flac::{self, drflac_open_memory_with_metadata};
    static FLAC_AUDIO: &[u8] = include_bytes!("../../../../assets/stereo.flac");
    drflac_open_memory_with_metadata(FLAC_AUDIO);
}
