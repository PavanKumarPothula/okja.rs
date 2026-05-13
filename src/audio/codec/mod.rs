mod dr_flac_bindings;

pub mod codec {
    // use crate::audio::codec::dr_flac_bindings::{
    //     drflac_meta_proc, drflac_open_memory_with_metadata, drflac_read_proc, drflac_seek_proc,
    // };
    use miniflac_sys::{FlacDecoder, StreamInfo};
    // use claxon::FlacReader;
    // use cty::;

    struct Metadata {
        title_name: &'static str,
        stream_info: StreamInfo,
    }

    struct MediaContainer {
        filename: &'static str,
        metadata: Metadata,
        byte_stream: [u8],
        decoder_obj: FlacDecoder,
    }
    enum Codec {
        FLAC(MediaContainer),
        // MP3,
        // AAC,
    }
    impl Codec {
        fn new(filename: &'static str, p_data_const: &'static [u8]) -> Self {
            let p_data = p_data_const.clone();
            match filename.rsplit_once(".").unwrap().1 {
                // "aac" => Self::AAC,
                // "mp3"   => Self::MP3,
                "flac" => {
                    // This actually can happen only once, instead of happening for every fileopen
                    let mut decoderObj = FlacDecoder::new();
                    decoderObj.init();
                    Self::FLAC(MediaContainer {
                        filename: filename,
                        metadata: Metadata {
                            title_name: filename,
                            stream_info: decoderObj.read_streaminfo(p_data).unwrap().1.unwrap(),
                        },
                        byte_stream: p_data.clone(),
                        decoder_obj: decoderObj,
                    })
                }
                _ => panic!("What!"),
            }
        }
        fn play(&self) {
            match self {
                Codec::FLAC(current_metadata_container) => {
                    if !current_metadata_container.byte_stream.is_empty() {
                        match current_metadata_container.decoder_obj.decode(current_metadata_container.byte_stream) {
                            Ok((consumed, Some(frame))) => {
                                let samples: &[i16] = frame.samples();
                                // process samples...
                                current_metadata_container.byte_stream =
                                    &current_metadata_container.byte_stream[consumed..];
                            }
                            Ok((consumed, None)) => {
                                current_metadata_container.byte_stream = &current_metadata_container.byte_stream[consumed.max(1)..]
                            }
                            Err(_) => todo!(),
                        }
                    }
                }
            }
        }
    }
}
