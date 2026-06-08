pub mod codec {

    use core::{ffi::c_void, i16, ptr};
    use cty;
    use defmt::info;
    use heapless::{String, Vec};
    // use crate::audio::codec::dr_flac_bindings::{
    //     drflac_meta_proc, drflac_open_memory_with_metadata, drflac_read_proc, drflac_seek_proc,
    // };
    // use miniflac_sys::{DecodedFrame, FlacDecoder, PictureInfo, StreamInfo, VorbisComments};

    use crate::audio::dr_flac_bindings::{
        self, drflac, drflac_allocation_callbacks, drflac_frame, drflac_int16, drflac_metadata,
        drflac_open_memory, drflac_open_memory_with_metadata, drflac_streaminfo, drflac_uint64,
        drflac_vorbis_comment_iterator,
    };

    // use claxon::flacreader;
    // use cty::;

    unsafe extern "C" {
        pub fn malloc(size: usize) -> *mut u8;

        pub fn malloc_internal(size: usize) -> *mut u8;

        pub fn free(ptr: *mut u8);

        pub fn free_internal(ptr: *mut u8);

        pub fn realloc_internal(ptr: *mut u8, size: usize) -> *mut u8;

        pub fn calloc(number: u32, size: usize) -> *mut u8;

        pub fn calloc_internal(number: u32, size: usize) -> *mut u8;

        pub fn get_free_internal_heap_size() -> usize;
    }

    // Wrapper pAllocCallbacks
    #[unsafe(no_mangle)]
    extern "C" fn my_malloc(sz: usize, pUserData: *mut cty::c_void) -> *mut cty::c_void {
        unsafe { malloc(sz) as *mut c_void }
    }

    #[unsafe(no_mangle)]
    extern "C" fn my_realloc(
        p: *mut cty::c_void,
        sz: usize,
        pUserData: *mut cty::c_void,
    ) -> *mut cty::c_void {
        unsafe { realloc_internal(p as *mut u8, sz) as *mut c_void }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn my_free(p: *mut cty::c_void, pUserData: *mut cty::c_void) {
        unsafe {
            free(p as *mut u8);
        }
    }
    pub struct DecoderResult {
        pub is_eof: bool,
        pub memory_pos: usize,
        // pub decoded_frame: Option<DecodedFrame>,
        pub decoded_frame: dr_flac_bindings::drflac_frame,
    }
    // #[derive(defmt::format)]
    #[derive(Default, Debug)]
    pub struct Metadata {
        pub audio_frame_start_pos: usize,
        pub title_name: heapless::String<256>,
        pub album_name: heapless::String<256>,
        pub album_artist: heapless::String<256>,
        pub stream_info: Option<drflac_streaminfo>,
        pub stream_info_size: usize,
        pub vorbis_comments_size: usize,
        // pub picture_data: Option<Vec<u8, 2048>>,
        // pub picture_info: Option<PictureInfo<32, 64>>,
    }
    // struct bytestreamcontainer{
    //     file_offset: usize,
    //     byte_stream_data: &'static [u8],
    // }

    pub struct MediaContainer {
        pub filename: &'static str,
        pub metadata: Metadata,
        pub decoder_obj: *mut drflac,
        // byte_stream_container : bytestreamcontainer,
    }

    pub enum Decoder {
        FLAC(MediaContainer),
        // mp3,
        // aac,
    }
    // impl defmt::format for mediacontainer {
    //     fn format(&self, fmt: defmt::formatter) {
    //         defmt::write!(
    //             fmt,
    //             "mediacontainer:: filename: {}:: metadata: {}",
    //             &self.filename,
    //             &self.metadata,
    //         )
    //     }
    // }

    impl Decoder {
        pub fn new(filename: &'static str, p_data_const: &'static [u8]) -> Self {
            info!(
                "Got: filename: {}, file_bytes: {}",
                filename,
                p_data_const.len()
            );
            match filename.rsplit_once(".").unwrap().1 {
                // "aac" => self::aac,
                // "mp3"   => self::mp3,
                "flac" => {
                    // this actually can happen only once, instead of happening for every fileopen
                    // unsafe { drflac_open_memory_with_metadata(p_data_const,p_data_const.len(),Some(on_meta_read),cty::NULL ,pAllocCallbacks); };

                    unsafe {
                        let decoder_obj = drflac_open_memory(
                            p_data_const.as_ptr() as *const c_void,
                            p_data_const.len(),
                            &drflac_allocation_callbacks {
                                pUserData: ptr::null::<u8>() as *mut c_void,
                                onMalloc: Some(my_malloc),
                                onRealloc: Some(my_realloc),
                                onFree: Some(my_free),
                            } as *const drflac_allocation_callbacks,
                        );
                        // info! {"totalPCMFrameCount:{}",
                        // (*decoder_obj).totalPCMFrameCount};
                        // let meta = gather_metadata(&mut decoder_obj, p_data_const);
                        Self::FLAC(MediaContainer {
                            filename: filename,
                            metadata: Metadata::default(),
                            decoder_obj,
                        })
                    }
                }
                _ => panic!("what!"),
            }
        }
        pub fn get_pcm_samples(&mut self, frames_to_read: u64, pcm_frames: &mut [i16]) -> u64 {
            match self {
                Decoder::FLAC(current_metadata_container) => unsafe {
                    let frames_read = dr_flac_bindings::drflac_read_pcm_frames_s16(
                        current_metadata_container.decoder_obj,
                        frames_to_read,
                        pcm_frames.as_mut_ptr() as *mut drflac_int16,
                    );
                    info! {"pBuffOut is zero?:{}", pcm_frames.iter().all(|&x| x == 0)};
                    info! {"pBuffOut :{}", pcm_frames};
                    frames_read
                },
            }
        }
        //     pub fn get_pcm_samples(&mut self, byte_stream: &[u8], mut pos: usize) -> DecoderResult {
        //         match self {
        //             Decoder::FLAC(current_metadata_container) => {
        //                 // info!("Pre-Sync Position : {}", pos);
        //                 // let sync_result = current_metadata_container
        //                 //     .decoder_obj
        //                 //     .sync(&byte_stream[pos..]);
        //                 let (next_frame_offset, is_next_frame_available) = sync_loop(
        //                     &mut current_metadata_container.decoder_obj,
        //                     byte_stream,
        //                     pos,
        //                 );
        //                 // let (next_frame_pos, is_next_frame_available) = sync_result
        //                 //     .inspect_err(|this_error| {
        //                 //         info!("error says {:#?}", defmt::Debug2Format(&this_error))
        //                 //     })
        //                 //     .unwrap();
        //                 if (is_next_frame_available) {
        //                     // pos += next_frame_offset;
        //                 }
        //                 info!("Post-Sync Position : {}", pos);
        //                 info!("bytestreamsize: {}", byte_stream.len());
        //                 info!("bytestreampos: {}", pos);
        //                 // let stream_info = current_metadata_container
        //                 //     .decoder_obj
        //                 //     .read_streaminfo(byte_stream);
        //                 match current_metadata_container
        //                     .decoder_obj
        //                     .decode(&byte_stream[pos..])
        //                     .inspect_err(|this_error| {
        //                         info!("error says {:#?}", defmt::Debug2Format(&this_error))
        //                     })
        //                     .unwrap()
        //                 {
        //                     (consumed, Some(f)) => {
        //                         pos += consumed;
        //                         return DecoderResult {
        //                             is_eof: false,
        //                             memory_pos: pos,
        //                             decoded_frame: Some(f),
        //                         };
        //                     }
        //                     (consumed, none) => {
        //                         if consumed == 0 {
        //                             return DecoderResult {
        //                                 is_eof: true,
        //                                 memory_pos: pos,
        //                                 decoded_frame: none,
        //                             };
        //                         } else {
        //                             pos += consumed;
        //                             return DecoderResult {
        //                                 is_eof: false,
        //                                 memory_pos: pos,
        //                                 decoded_frame: none,
        //                             };
        //                         }
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // }

        // fn gather_metadata(decoder_obj: &mut FlacDecoder, p_data_const: &[u8]) -> Metadata {
        //     let mut final_pos = 0;
        //     let (stream_info_size, stream_info) = decoder_obj.read_streaminfo(p_data_const).unwrap();
        //     final_pos += stream_info_size;
        //     info!("Got StreamInfo of size : {}", stream_info_size);
        //     let mut meta = Metadata {
        //         title_name: String::new(),
        //         album_artist: String::new(),
        //         album_name: String::new(),
        //         stream_info: stream_info.unwrap(),
        //         stream_info_size: stream_info_size,
        //         vorbis_comments_size: 0_usize,
        //         audio_frame_start_pos: 0_usize,
        //         picture_data: None,
        //         picture_info: None,
        //     };
        //     //     check_meta_info(decoder_obj, p_data_const, &mut final_pos, &mut meta);
        //     //     (final_pos, meta)
        //     // }
        //     // fn check_meta_info(
        //     //     decoder_obj: &mut FlacDecoder,
        //     //     p_data_const: &[u8],
        //     //     final_pos: &mut usize,
        //     //     meta: &mut metadata,
        //     // ) {
        //     // let (consumed_bytes, result) = decoder_obj.sync(&p_data_const[*final_pos..]).unwrap();
        //     // if result {
        //     //     **final_pos += consumed_bytes;
        //     // }
        //     // let (vorbois_comments_size, vorbis_comments) = decoder_obj
        //     //     .read_vorbis_comments::<128, 256, 16>(&p_data_const[*final_pos..])
        //     //     .inspect_err(|this_error| info!("Error says {:#?}", defmt::Debug2Format(&this_error)))
        //     //     .unwrap();
        //     // let vorbis_comments = vorbis_comments.unwrap().comments;
        //     // vorbis_comments.iter().for_each(|line| {
        //     //     let (key, value) = core::str::from_utf8(&line)
        //     //         .unwrap()
        //     //         .split_once("=")
        //     //         .unwrap();
        //     //     match key {
        //     //         "TITLE" => meta.title_name.push_str(value).unwrap(),
        //     //         "ALBUM" => meta.album_name.push_str(value).unwrap(),
        //     //         "ALBUMARTIST" => meta.album_artist.push_str(value).unwrap(),
        //     //         _ => (),
        //     //     }
        //     // });
        //     let mut comments: Option<VorbisComments<128, 256, 16>> = None;
        //     let mut picture_data: Option<Vec<u8, 2048>> = None;
        //     let mut picture_info: Option<PictureInfo<32, 64>> = None;
        //
        //     loop {
        //         // Sync to next metadata/frame boundary
        //         // let (consumed, ready) = decoder_obj
        //         //     .sync(&p_data_const[final_pos..])
        //         //     .inspect_err(|this_error| {
        //         //         info!("error says {:#?}", defmt::Debug2Format(&this_error))
        //         //     })
        //         //     .unwrap();
        //         let (next_frame_offset, is_next_frame_available) =
        //             sync_loop(decoder_obj, p_data_const, final_pos);
        //         if !is_next_frame_available {
        //             info!("End of Metablocks");
        //             break;
        //         }
        //         final_pos += next_frame_offset;
        //         info!("Post-Sync Position : {}", final_pos);
        //
        //         // Try reading as vorbis comments
        //         if comments.is_none() {
        //             match decoder_obj.read_vorbis_comments::<128, 256, 16>(&p_data_const[final_pos..]) {
        //                 Ok((consumed, Some(vc))) => {
        //                     final_pos += consumed;
        //                     comments = Some(vc);
        //                     info!("Got vorbois comments of size : {}", consumed);
        //                     meta.vorbis_comments_size += consumed;
        //                     comments.clone().unwrap().comments.iter().for_each(|line| {
        //                         let (key, value) = core::str::from_utf8(&line)
        //                             .unwrap()
        //                             .split_once("=")
        //                             .unwrap();
        //                         match key {
        //                             "TITLE" => meta.title_name.push_str(value).unwrap(),
        //                             "ALBUM" => meta.album_name.push_str(value).unwrap(),
        //                             "ALBUMARTIST" => meta.album_artist.push_str(value).unwrap(),
        //                             _ => (),
        //                         }
        //                     });
        //                     continue;
        //                 }
        //                 Ok((consumed, None)) => {
        //                     final_pos += consumed;
        //                     meta.vorbis_comments_size += consumed;
        //                     continue;
        //                 }
        //                 Err(_) => {
        //                     info!("not a vorbis comment block, try picture");
        //                 }
        //             }
        //         }
        //
        //         // Try reading as picture
        //         if picture_info.is_none() {
        //             match decoder_obj.read_picture_info::<32, 64>(&p_data_const[final_pos..]) {
        //                 Ok((consumed, Some(pi))) => {
        //                     final_pos += consumed;
        //                     // Read the image p_data_const immediately after info
        //                     let mut buf = Vec::<u8, 2048>::new();
        //                     buf.fill(0);
        //                     // vec![0u8; pi.data_length as usize];
        //                     let (consumed, picture_data_size) = decoder_obj
        //                         .read_picture_data(&p_data_const[final_pos..], &mut buf)
        //                         .unwrap();
        //                     final_pos += consumed;
        //                     info!("Got PictureInfo of size : {}", consumed);
        //                     info!("Got PictureData of size : {}", picture_data_size);
        //                     picture_data = Some(buf);
        //                     picture_info = Some(pi);
        //                     meta.picture_info = picture_info.clone();
        //                     meta.picture_data = picture_data;
        //                     continue;
        //                 }
        //                 Ok((consumed, None)) => {
        //                     final_pos += consumed;
        //                     continue;
        //                 }
        //                 Err(_) => {
        //                     info!("not a picture block (padding, seektable, etc.)");
        //                 }
        //             }
        //         }
        //
        //         // Unknown/unhandled block type -- try to decode as audio.
        //         // If decode returns a frame, we've hit audio p_data_const.
        //         match decoder_obj.decode(&p_data_const[final_pos..]) {
        //             Ok((_, Some(_frame))) => {
        //                 info!("Reached audio frames -- metadata is done.");
        //                 break;
        //             }
        //             Ok((consumed, None)) => {
        //                 final_pos += consumed;
        //             }
        //             Err(_) => {
        //                 info!("Okay, you've hit the unicorn block now!");
        //                 //Jugaad to skip the block
        //                 // final_pos+=1;
        //                 break;
        //             }
        //         }
        //     }
        //     meta.audio_frame_start_pos = final_pos;
        //     meta
        // }
        // /// Helper: sync past metadata blocks until we reach one we want, or audio frames.
        // /// Returns (new_offset, synced) where synced=true means sync succeeded.
        // fn sync_loop(dec: &mut FlacDecoder, data: &[u8], mut pos: usize) -> (usize, bool) {
        //     while pos < data.len() {
        //         match dec
        //             .sync(&data[pos..])
        //             .inspect_err(|this_error| {
        //                 info!("error says {:#?}", defmt::Debug2Format(&this_error))
        //             })
        //             .unwrap()
        //         {
        //             (consumed, true) => {
        //                 pos += consumed;
        //                 return (pos, true);
        //             }
        //             (consumed, false) => {
        //                 if consumed == 0 {
        //                     break;
        //                 }
        //                 pos += consumed;
        //             }
        //         }
        //     }
        //     (pos, false)
        // }
        //
        // unsafe extern "C" fn on_meta_read(
        //     p_user_data: *mut cty::c_void,
        //     p_metadata: *mut drflac_metadata,
        // ) {
        //     p_user_data();
        // }
    }
}
