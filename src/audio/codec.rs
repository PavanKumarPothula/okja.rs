pub mod codec {

    use defmt::info;
    use field::field;
    use heapless::String;
    // use crate::audio::codec::dr_flac_bindings::{
    //     drflac_meta_proc, drflac_open_memory_with_metadata, drflac_read_proc, drflac_seek_proc,
    // };
    use miniflac_sys::{DecodedFrame, FlacDecoder, StreamInfo};

    // use claxon::FlacReader;
    // use cty::;
    pub struct DecoderResult {
        pub is_eof: bool,
        pub memory_pos: usize,
        pub decoded_frame: Option<DecodedFrame>,
    }
    // #[derive(defmt::Format)]
    pub struct Metadata {
        pub title_name: heapless::String<256>,
        pub album_name: heapless::String<256>,
        pub album_artist: heapless::String<256>,
        pub stream_info: StreamInfo,
        pub metadata_size: usize,
        pub vorbois_comments_size: usize,
    }
    // struct ByteStreamContainer{
    //     file_offset: usize,
    //     byte_stream_data: &'static [u8],
    // }

    pub struct MediaContainer {
        pub filename: &'static str,
        pub metadata: Metadata,
        pub decoder_obj: FlacDecoder,
        // byte_stream_container : ByteStreamContainer,
    }

    pub enum Decoder {
        FLAC(MediaContainer),
        // MP3,
        // AAC,
    }
    // impl defmt::Format for MediaContainer {
    //     fn format(&self, fmt: defmt::Formatter) {
    //         defmt::write!(
    //             fmt,
    //             "MediaContainer:: filename: {}:: metadata: {}",
    //             &self.filename,
    //             &self.metadata,
    //         )
    //     }
    // }

    impl Decoder {
        pub fn new(filename: &'static str, p_data_const: &'static [u8]) -> Self {
            match filename.rsplit_once(".").unwrap().1 {
                // "aac" => Self::AAC,
                // "mp3"   => Self::MP3,
                "flac" => {
                    // This actually can happen only once, instead of happening for every fileopen
                    let mut decoder_obj = FlacDecoder::new();
                    decoder_obj.init();
                    let (metadata_size, metadata) =
                        decoder_obj.read_streaminfo(p_data_const).unwrap();
                    let (vorbois_comments_size, vorbis_comments) = decoder_obj
                        .read_vorbis_comments::<128, 256, 16>(&p_data_const[metadata_size..])
                        .unwrap();
                    let vorbis_comments = vorbis_comments.unwrap().comments;
                    let mut meta = Metadata {
                        title_name: String::new(),
                        album_artist: String::new(),
                        album_name: String::new(),
                        stream_info: metadata.unwrap(),
                        metadata_size: metadata_size,
                        vorbois_comments_size: vorbois_comments_size,
                    };
                    &vorbis_comments
                        .iter()
                        .map(|line| {
                            core::str::from_utf8(&line)
                                .unwrap()
                                .split_once("=")
                                .unwrap()
                        })
                        .map(|(key, value)| match key {
                            "TITLE" => meta.title_name.push_str(value).unwrap(),
                            "ALBUM" => meta.album_name.push_str(value).unwrap(),
                            "ALBUMARTIST" => meta.album_artist.push_str(value).unwrap(),
                            _ => todo!(),
                        });
                    Self::FLAC(MediaContainer {
                        filename: filename,
                        metadata: meta,
                        decoder_obj,
                    })
                }
                _ => panic!("What!"),
            }
        }
        pub fn get_pcm_samples(&mut self, byte_stream: &[u8], mut pos: usize) -> DecoderResult {
            match self {
                Decoder::FLAC(current_metadata_container) => {
                    // let sync_result = current_metadata_container.decoder_obj.sync(byte_stream);
                    // let (next_frame_pos, is_next_frame_available) = sync_result
                    //     .inspect_err(|this_error| {
                    //         info!("Error says {:#?}", defmt::Debug2Format(&this_error))
                    //     })
                    //     .unwrap();
                    // if is_next_frame_available {
                    //     pos += next_frame_pos;
                    //     info!("ByteStreamNextBoundaryOffset: {}", next_frame_pos);
                    // }
                    info!("ByteStreamSize: {}", byte_stream.len());
                    info!("ByteStreamPos: {}", pos);
                    // let stream_info = current_metadata_container
                    //     .decoder_obj
                    //     .read_streaminfo(byte_stream);
                    match current_metadata_container
                        .decoder_obj
                        .decode(&byte_stream[pos..])
                        .inspect_err(|this_error| {
                            info!("Error says {:#?}", defmt::Debug2Format(&this_error))
                        })
                        .unwrap()
                    {
                        (consumed, Some(f)) => {
                            pos += consumed;
                            return DecoderResult {
                                is_eof: false,
                                memory_pos: pos,
                                decoded_frame: Some(f),
                            };
                        }
                        (consumed, None) => {
                            if consumed == 0 {
                                return DecoderResult {
                                    is_eof: true,
                                    memory_pos: pos,
                                    decoded_frame: None,
                                };
                            } else {
                                pos += consumed;
                                return DecoderResult {
                                    is_eof: false,
                                    memory_pos: pos,
                                    decoded_frame: None,
                                };
                            }
                        }
                    }
                }
            }
        }
    }
}
