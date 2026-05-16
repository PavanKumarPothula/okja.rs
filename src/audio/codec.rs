pub mod codec {
    use defmt::info;
    // use crate::audio::codec::dr_flac_bindings::{
    //     drflac_meta_proc, drflac_open_memory_with_metadata, drflac_read_proc, drflac_seek_proc,
    // };
    use miniflac_sys::{DecodedFrame, FlacDecoder, StreamInfo};

    // use claxon::FlacReader;
    // use cty::;

    struct Metadata {
        title_name: &'static str,
        stream_info: StreamInfo,
    }
    // struct ByteStreamContainer{
    //     file_offset: usize,
    //     byte_stream_data: &'static [u8],
    // }
    struct MediaContainer {
        filename: &'static str,
        metadata: Metadata,
        decoder_obj: FlacDecoder,
        // byte_stream_container : ByteStreamContainer,
    }
    pub enum Decoder {
        FLAC(MediaContainer),
        // MP3,
        // AAC,
    }
    impl Decoder {
        pub fn new(filename: &'static str, p_data_const: &'static [u8]) -> Self {
            match filename.rsplit_once(".").unwrap().1 {
                // "aac" => Self::AAC,
                // "mp3"   => Self::MP3,
                "flac" => {
                    // This actually can happen only once, instead of happening for every fileopen
                    let mut decoder_obj = FlacDecoder::new();
                    decoder_obj.init();
                    Self::FLAC(MediaContainer {
                        filename: filename,
                        metadata: Metadata {
                            title_name: filename,
                            stream_info: decoder_obj
                                .read_streaminfo(p_data_const)
                                .unwrap()
                                .1
                                .unwrap(),
                        },
                        decoder_obj, // byte_stream_container: ByteStreamContainer { file_offset: 0, byte_stream_data: &[0_u8;] }
                    })
                }
                _ => panic!("What!"),
            }
        }
        pub fn get_pcm_samples(self, byte_stream: &[u8]) -> DecodedFrame {
            match self {
                Decoder::FLAC(mut current_metadata_container) => current_metadata_container
                    .decoder_obj
                    .decode(byte_stream)
                    .inspect_err(|this_error| info!("{:#?}", defmt::Debug2Format(&this_error)))
                    .unwrap()
                    .1
                    .unwrap(),
            }
        }
    }
}
