mod dr_bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
mod flac {
    use crate::audio::codec::{
        codec::{Decoder, Demuxer, MediaContainer, Metadata},
        flac::dr_bindings,
    };

    struct FlacContainer(str);
    impl Demuxer for FlacContainer {
        fn new(filename: &str) -> Self {
            FlacContainer(MediaContainer {
                filename,
                metadata: Self::get_metadata(Self),
                pcm_samples: [0;128],
            })
        }
        fn get_metadata(self) -> crate::audio::codec::codec::Metadata {
            dr_bindings::drflac_open_memory_with_metadata();
        }
    }
}
