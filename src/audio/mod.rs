pub mod codec;
pub mod player;

use core::slice::from_raw_parts;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer, block_for};
use esp_backtrace as _;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::Config as I2CConfig;
use esp_hal::i2s::master::{Channels, Config as I2SConfig, DataFormat, I2s, I2sTx, UnitConfig};
use esp_hal::time::Rate;
use esp_hal::{Blocking, dma, dma_circular_buffers, i2c};
// use esp_println::{self as _, info};
use defmt::info;
use defmt_rtt as _;

// use mousefood::ratatui::Terminal;
// use mousefood::*;

use miniflac_sys::FlacDecoder;
use tlv320dac3100::TLV320DAC3100;
use tlv320dac3100::typedefs::*;

use crate::audio::codec::codec::Decoder;
use crate::{DACPeripherals, DACResources};

pub fn init(r: DACPeripherals<'static>) -> DACResources {
    info!("Audio init Start!");
    let mut rst_pin = Output::new(r.dac_rst, Level::Low, OutputConfig::default());
    rst_pin.set_low();
    block_for(Duration::from_millis(200));
    rst_pin.set_high();
    let i2c_bus_instance = i2c::master::I2c::new(r.i2c_module, I2CConfig::default())
        .unwrap()
        .with_scl(r.i2c_scl)
        .with_sda(r.i2c_sda);
    info!("Audio I2C Bus Start!");

    let mut dac_obj = TLV320DAC3100::new(i2c_bus_instance);

    dac_obj
        .set_codec_interface_control_1(
            CodecInterface::I2S,
            CodecInterfaceWordLength::Word16Bits,
            false,
            false,
        )
        .expect("Error setting codec interface control 1");
    dac_obj
        .set_clock_gen_muxing(PllClkin::Bclk, CodecClkin::PllClk)
        .expect("Error setting clock gen muxing");
    // TODO PLL is powered later in ref example
    dac_obj.set_pll_p_and_r_values(true, 2, 2).expect("");
    dac_obj.set_pll_j_value(32).expect("Error setting pll j");
    dac_obj.set_pll_d_value(0).expect("Error setting pll d");
    dac_obj
        .set_dac_ndac_val(true, 8)
        .expect("Error setting dac NDAC val");
    dac_obj
        .set_dac_mdac_val(true, 2)
        .expect("Error setting dac MDAC val");

    dac_obj
        .set_dac_data_path_setup(
            true,
            true,
            LeftDataPath::Left,
            RightDataPath::Right,
            SoftStepping::OneStepPerTwoPeriods,
        )
        .expect("Error setting dac DataPath setup");

    dac_obj
        .set_dac_l_and_dac_r_output_mixer_routing(
            DacLeftOutputMixerRouting::LeftChannelMixerAmplifier,
            false,
            false,
            DacRightOutputMixerRouting::RightChannelMixerAmplifier,
            false,
            false,
        )
        .expect("Error setting dac L and R mixer routing");
    dac_obj
        .set_dac_volume_control(false, false, VolumeControl::IndependentChannels)
        .expect("Error setting dac Volume control");

    dac_obj
        .set_dac_left_volume_control(8.0)
        .expect("Error setting dac Left volume control");
    dac_obj
        .set_dac_right_volume_control(8.0)
        .expect("Error setting dac Right volume control");
    dac_obj
        .set_headphone_drivers(true, true, HpOutputVoltage::Common1_35V, false)
        .expect("Error setting dac Headphone drivers");

    dac_obj
        .set_hpl_driver(0, true)
        .expect("Error setting dac hpl driver");
    dac_obj
        .set_hpr_driver(0, false)
        .expect("Error setting dac hpr driver");
    dac_obj
        .set_left_analog_volume_to_hpl(true, 0)
        .expect("Error setting left analog volume to hpl");
    dac_obj
        .set_right_analog_volume_to_hpr(true, 0)
        .expect("Error setting right analog volume to hpr");
    dac_obj
        .set_class_d_spk_amp(true)
        .expect("Error setting class D spk amp");
    dac_obj
        .set_left_analog_volume_to_spk(true, 0)
        .expect("Error setting left analog volume to spk");
    dac_obj
        .set_class_d_spk_driver(OutputStage::Gain6dB, false)
        .expect("Error setting class D spk driver");

    // // // dac_obj
    // // //     .set_micbias(false, true, MicBiasOutput::PoweredAVDD)
    // //     .expect("Error setting micbias");
    // dac_obj
    //     .set_headset_detection(
    //         true,
    //         HeadsetDetectionDebounce::Debounce16ms,
    //         HeadsetButtonPressDebounce::Debounce0ms,
    //     )
    //     .expect("Error setting headset detection");
    dac_obj
        .set_int1_control_register(true, true, false, false, false, false)
        .expect("Error setting int1 control register");
    dac_obj
        .set_gpio1_io_pin_control(Gpio1Mode::Int1)
        .expect("Error setting gpio1 io pin");
    block_for(Duration::from_millis(1000));
    info!("Audio init End!");
    DACResources {
        tlv_obj: dac_obj,
        i2s_bclk: r.i2s_bclk,
        i2s_dma: r.i2s_dma,
        i2s_dout: r.i2s_dout,
        i2s_module: r.i2s_module,
        i2s_ws: r.i2s_ws,
    }
}

#[embassy_executor::task]
pub async fn play_pause(decoder: Decoder, i2s_tx_writer: I2sTx<'static, Blocking>) {
    let pos = 0;
    match decoder {
        codec::codec::Decoder::FLAC(ref this_meta) => {
            let pos = this_meta.metadata.metadata_size;
            i2s_tx_writer
                .apply_config(
                    &UnitConfig::new_tdm_philips()
                        .with_channels(Channels::STEREO)
                        .with_data_format(DataFormat::Data16Channel16)
                        .with_sample_rate(Rate::from_hz(
                            this_meta.metadata.stream_info.sample_rate,
                        )), // .with_data_format()
                )
                .unwrap();
        }
    }
    let mut transfer = i2s_tx_writer.write_dma_circular(dma_tx_buf).unwrap();

    while pos <= FLAC_AUDIO.len() {
        // info!("AUDIOTASK: Position:{}", pos);
        let decoder_result = decoder.get_pcm_samples(&FLAC_AUDIO, pos);
        // info!("AUDIOTASK: Consumed:{}", decoder_result.memory_pos - pos);
        // info!("AUDIOTASK: isEOF:{}", decoder_result.is_eof);
        if !decoder_result.is_eof {
            let frame_result = decoder_result.decoded_frame;
            pos = decoder_result.memory_pos; // for the next decode op
            if frame_result.is_none() {
                // info!("AUDIOTASK: NOFRAME");
                continue;
            }
            let frame = frame_result.unwrap();
            // info!("AUDIOTASK: SampleNumber:{}", frame.sample_number);
            // info!("AUDIOTASK: SamplesRate:{}", frame.sample_rate);
            // info!("AUDIOTASK: Bps:{}", frame.bps);
            // info!("AUDIOTASK: Channels:{}", frame.channels);
            // i2s_tx_writer.apply_config(&UnitConfig::default().with_ws_width(ws_width))
            let samples_to_write = frame.samples();
            let frame_size_bytes = samples_to_write.len().checked_mul(2).unwrap();
            loop {
                let dma_available_bytes = transfer
                    .available()
                    .inspect_err(|e: &dma::DmaError| info!("DMAError: {}", e))
                    .unwrap();
                info!("AUDIOTASK: Available Bytes: {}", dma_available_bytes);
                if dma_available_bytes < frame_size_bytes {
                    Timer::after(Duration::from_nanos(10)).await;
                } else {
                    transfer
                        .push(unsafe {
                            from_raw_parts(samples_to_write.as_ptr().cast(), frame_size_bytes)
                        })
                        .inspect_err(|e| info!("DMAError: {}", e))
                        .unwrap();
                    break;
                }
            }
            // transfer.push_with(|_|frame.samples().len()).unwrap();
        } else {
            break;
        }
    }
}
#[embassy_executor::task]
pub async fn main_task(spawner: Spawner, dac_peripherals: DACResources) {
    info!("AUDIOTASK: Audio Started");
    const SINE_WAVE: [i16; 96] = [
        0, 0, 4277, 4277, 8481, 8481, 12539, 12539, 16383, 16383, 19947, 19947, 23170, 23170,
        25996, 25996, 28377, 28377, 30273, 30273, 31650, 31650, 32487, 32487, 32767, 32767, 32487,
        32487, 31650, 30273, 30273, 28377, 28377, 25996, 25996, 23170, 23170, 19947, 19947, 16383,
        16383, 12539, 12539, 8481, 8481, 4277, 4277, 0, 0, -4277, -4277, -8481, -8481, -12539,
        -16383, -16383, -19947, -19947, -23170, -23170, -25996, -25996, -28377, -28377, -30273,
        -30273, -31650, -31650, -32487, -32487, -32767, -32767, -32487, -32487, -31650, -30273,
        -30273, -28377, -28377, -25996, -25996, -23170, -23170, -19947, -19947, -16383, -16383,
        -12539, -12539, -8481, -8481, -4277, -4277, -4277, -4277, -4277,
    ];
    let mut pos = 0;

    let i2s_driver = I2s::new(
        dac_peripherals.i2s_module,
        dac_peripherals.i2s_dma,
        I2SConfig::new_tdm_philips()
            .with_bit_order(esp_hal::i2s::master::BitOrder::MsbFirst)
            .with_sample_rate(Rate::from_khz(48)),
    )
    .unwrap();
    let (_, _, dma_tx_buf, dma_tx_desc) = dma_circular_buffers!(0, 147456);
    let mut i2s_tx_writer = i2s_driver
        .i2s_tx
        .with_bclk(dac_peripherals.i2s_bclk)
        .with_dout(dac_peripherals.i2s_dout)
        .with_ws(dac_peripherals.i2s_ws)
        .build(dma_tx_desc);
    let _index = 0;
    for pair in dma_tx_buf.chunks_mut(2) {
        [pair[0], pair[1]] = [0, 0];
        // [pair[0], pair[1]] = SINE_WAVE[index % 96].to_ne_bytes();
        // index += 1;
    }

    static AUDIO_FILENAME: &str = "stereo.flac";
    static FLAC_AUDIO: &[u8] = include_bytes!("../../assets/stereo.flac");
    let mut decoder = codec::codec::Decoder::new(AUDIO_FILENAME, FLAC_AUDIO);
    play_pause(decoder, i2s_tx_writer);
}
