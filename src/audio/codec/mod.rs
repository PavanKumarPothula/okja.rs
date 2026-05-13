#![feature(default_field_values)]
pub mod flac;

pub mod codec {
    use core::fmt::Error;

    pub struct Metadata {
        pub title_name: &'static str,
        pub channels: usize,
        pub sample_count: usize,
        pub sample_rate: u32,
    }
    pub struct MediaContainer {
        pub filename: &'static str,
        pub metadata: Metadata,
        pub pcm_samples: [usize],
    }
    pub trait Decoder {
        fn new(filename: &str) -> Self;
        fn get_metadata(self) -> Metadata;
        fn get_pcm_samples(self) -> [usize];
    }

    enum Codec {
        FLAC,
        MP3,
        AAC,
    }
    impl Decoder for Codec {
        fn new(filename: &str) -> Self{
            match filename
            .rsplit_once(".")
            .unwrap().1 {
                "aac" => Self::AAC,
                "flac" => Self::FLAC,
                "mp3"   => Self::MP3,
                _ => panic!("What!")
            }
        }
    }
}
