pub mod codec {

    use core::{alloc::GlobalAlloc, ffi::c_void, i16, ptr};
    use critical_section::with;
    use cty;
    use defmt::info;
    use heapless::{String, Vec};

    use crate::audio::dr_flac_bindings::{
        self, drflac, drflac_allocation_callbacks, drflac_int16, drflac_open_memory,
        drflac_streaminfo,
    };

    unsafe fn malloc_8_bytes_aligned_memory(size: usize) -> *mut u8 {
        let total_size = size + 8;

        unsafe {
            let ptr = esp_alloc::HEAP.alloc_caps(
                esp_alloc::export::enumset::EnumSet::empty(),
                core::alloc::Layout::from_size_align_unchecked(total_size, 8),
            );

            if ptr.is_null() {
                return ptr;
            }

            *(ptr as *mut usize) = total_size;
            ptr.offset(8)
        }
    }

    unsafe fn realloc_8_bytes_aligned_memory(ptr: *mut u8, new_size: usize) -> *mut u8 {
        unsafe extern "C" {
            fn memcpy(d: *mut u8, s: *const u8, l: usize);
        }

        unsafe {
            let p = malloc_8_bytes_aligned_memory(new_size);
            if !p.is_null() && !ptr.is_null() {
                let len = usize::min(
                    (ptr as *const u32).sub(1).read_volatile() as usize - 8,
                    new_size,
                );
                memcpy(p, ptr, len);
                free_8_byte_aligned_mem(ptr);
            }
            p
        }
    }
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn free_8_byte_aligned_mem(ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }

        unsafe {
            let ptr = ptr.offset(-8);
            let total_size = *(ptr as *const usize);
            esp_alloc::HEAP.dealloc(
                ptr,
                core::alloc::Layout::from_size_align_unchecked(total_size, 8),
            )
        }
    }

    // Wrapper pAllocCallbacks
    #[unsafe(no_mangle)]
    extern "C" fn my_malloc(sz: usize, pUserData: *mut cty::c_void) -> *mut cty::c_void {
        // let x = malloc(sz);
        let x = unsafe { malloc_8_bytes_aligned_memory(sz) };
        info! {"malloc addr: {}",defmt::Debug2Format(&x)};
        unsafe {
            info! {"malloc value: {}",defmt::Debug2Format(&(*x))}
        }
        x as *mut c_void
    }

    #[unsafe(no_mangle)]
    extern "C" fn my_realloc(
        p: *mut cty::c_void,
        sz: usize,
        pUserData: *mut cty::c_void,
    ) -> *mut cty::c_void {
        let x = unsafe { realloc_8_bytes_aligned_memory(p as *mut u8, sz) };
        info! {"realloc addr: {}",defmt::Debug2Format(&x)};
        unsafe {
            info! {"realloc value: {}",defmt::Debug2Format(&(*x))}
        }
        x as *mut c_void
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn my_free(p: *mut cty::c_void, pUserData: *mut cty::c_void) {
        unsafe { free_8_byte_aligned_mem(p as *mut u8) };
        info! {"free addr: {}",defmt::Debug2Format(&p)};
    }

    pub struct DecoderResult {
        pub is_eof: bool,
        pub currentPCMFrameIdx: u64,
        pub framesRead: u64,
    }

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
                        info! {"totalPCMFrameCount:{}",
                        (*decoder_obj).totalPCMFrameCount};
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
        pub fn get_pcm_samples(
            &mut self,
            frames_to_read: u64,
            pcm_frames: &mut [i16],
        ) -> DecoderResult {
            match self {
                Decoder::FLAC(current_metadata_container) => unsafe {
                    let frames_read = dr_flac_bindings::drflac_read_pcm_frames_s16(
                        current_metadata_container.decoder_obj,
                        frames_to_read,
                        pcm_frames.as_mut_ptr() as *mut drflac_int16,
                    );
                    info! {"pBuffOut is zero?:{}", pcm_frames.iter().all(|&x| x == 0)};

                    // info! {"pBuffOut :{}", pcm_frames};
                    DecoderResult {
                        framesRead: frames_read,
                        currentPCMFrameIdx: current_metadata_container
                            .decoder_obj
                            .as_ref()
                            .unwrap()
                            .currentPCMFrame,
                        is_eof: false,
                    }
                },
            }
        }
    }
}
