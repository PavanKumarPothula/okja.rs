pub mod flac;

pub mod codec {
    pub struct Metadata {
        pub title_name: &'static str,
        pub channels: usize,
        pub sample_count: usize,
        pub sample_rate: u32,
    }
    pub struct Decoder {
        pub filename: &'static str,
        pub metadata: Metadata,
        pub pcm_samples: [usize],
    }
    trait DecoderTrait {
        fn new(filename: &str)-> Self;
    }

}