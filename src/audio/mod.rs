pub(crate) mod codec;
pub(crate) mod dr_flac_bindings;
pub mod player;

use core::cmp::min;
use core::ops::Index;
use core::slice::from_raw_parts;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, block_for};
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

use heapless::Vec;
use tlv320dac3100::TLV320DAC3100;
use tlv320dac3100::typedefs::*;

use crate::audio::codec::codec::Decoder;
use crate::{DACPeripherals, DACResources};

#[derive(Clone, Copy, Debug)]
pub struct FileInfo {
    pub file_name: &'static str,
    pub file_bytes: &'static [u8],
}
#[derive(Clone, Copy, Debug)]
pub enum PlayPauseState {
    Play,
    Pause,
}
pub static PLAY_PAUSE_STATE: Signal<CriticalSectionRawMutex, PlayPauseState> = Signal::new();
pub static AUDIO_DECODER: Signal<CriticalSectionRawMutex, FileInfo> = Signal::new();

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

struct I2SResources {
    i2s_tx_writer: I2sTx<'static, Blocking>,
    dma_tx_buf: &'static [u8; 32 * 1024],
}

#[embassy_executor::task]
pub async fn player_task(dac_peripherals: DACResources) {
    info!("AUDIOTASK: Audio Started");

    let i2s_driver = I2s::new(
        dac_peripherals.i2s_module,
        dac_peripherals.i2s_dma,
        I2SConfig::new_tdm_philips()
            .with_bit_order(esp_hal::i2s::master::BitOrder::MsbFirst)
            .with_sample_rate(Rate::from_khz(48)),
    )
    .unwrap();
    let (_, _, dma_tx_buf, dma_tx_desc) = dma_circular_buffers!(0, 32 * 1024);
    let i2s_tx_writer = i2s_driver
        .i2s_tx
        .with_bclk(dac_peripherals.i2s_bclk)
        .with_dout(dac_peripherals.i2s_dout)
        .with_ws(dac_peripherals.i2s_ws)
        .build(dma_tx_desc);
    let _index = 0;
    dma_tx_buf.fill(1);
    let mut i2s_resources = I2SResources {
        i2s_tx_writer,
        dma_tx_buf,
    };

    loop {
        info!("Waiting for the Decoder obj to come in");
        let FileInfo {
            file_name,
            file_bytes,
        } = AUDIO_DECODER.wait().await;
        info!("Got the FileInfo obj");
        let mut decoder = Decoder::new(file_name, file_bytes);
        match decoder {
            codec::codec::Decoder::FLAC(ref this_meta) => {
                // pos = this_meta.metadata.audio_frame_start_pos;
                info!("Metadata: {}", defmt::Debug2Format(&this_meta.metadata));
                i2s_resources
                    .i2s_tx_writer
                    .apply_config(
                        &UnitConfig::new_tdm_philips()
                            .with_channels(Channels::STEREO)
                            .with_data_format(DataFormat::Data16Channel16)
                            .with_sample_rate(Rate::from_hz(
                                // this_meta.metadata.stream_info.unwrap().sampleRate,
                                48000, // TODO: Change this to be variable
                            )), // .with_data_format()
                    )
                    .unwrap();

                info!("Configured the I2STx Writer");
            }
        }
        let mut transfer = i2s_resources
            .i2s_tx_writer
            .write_dma_circular(i2s_resources.dma_tx_buf)
            .unwrap();

        // let samples_to_write: Vec<i16, 512> = Vec::from([0; 512]);
        //
        const NUM_SAMPLES_PER_CALL: usize = 1024;
        let mut samples_to_write = [0_i16; NUM_SAMPLES_PER_CALL];
        let mut frame_size_bytes: usize;

        info!("Starting the buff filler");
        let mut last_player_state = PlayPauseState::Pause;
        while !AUDIO_DECODER.signaled() {
            // info!("AUDIOTASK: Position:{}", pos);
            // info!("AUDIOTASK: Consumed:{}", decoder_result.memory_pos - pos);
            // info!("AUDIOTASK: isEOF:{}", decoder_result.is_eof);
            let current_play_pause_state = if PLAY_PAUSE_STATE.signaled() {
                last_player_state = PLAY_PAUSE_STATE.wait().await;
                last_player_state
            } else {
                last_player_state
            };
            info!(
                "Current State:{}",
                defmt::Debug2Format(&current_play_pause_state)
            );
            match current_play_pause_state {
                PlayPauseState::Play => {
                    let frames_to_read = (NUM_SAMPLES_PER_CALL / 2) as u64;
                    let decoder_meta =
                        decoder.get_pcm_samples(frames_to_read, &mut samples_to_write);
                    info! {"FramesRead:{}",decoder_meta.framesRead};
                    info! {"currentSampleIdx:{}",decoder_meta.currentPCMFrameIdx};
                    if decoder_meta.framesRead == 0 {
                        info!("EOF breaking out");
                        break;
                    }
                }
                PlayPauseState::Pause => {
                    // samples_to_write.extend(repeat_n(0, 1000));
                    // samples_to_write.copy_from_slice(&[0;16*1024]);
                    // samples_to_write.fill(0_i16);
                    embassy_time::Timer::after(Duration::from_nanos(10)).await;
                    // info!("In Pause State: Filling Sending filled zeros");
                }
            }
            frame_size_bytes = samples_to_write.len() * 2;

            // info!(
            //     "AUDIOTASK: Bytes Contents: {}",
            //     defmt::Debug2Format(&samples_to_write)
            // );
            info!("AUDIOTASK: Bytes to Write: {}", frame_size_bytes);
            let mut chunk_start_index = 0;
            let mut chunk_end_index = 0;

            loop {
                // if true{break;}
                let dma_available_bytes = transfer
                    .available()
                    .inspect_err(|e: &dma::DmaError| info!("DMAError: {}", e))
                    .unwrap();
                if dma_available_bytes == 0 {
                    // info!("DMA full");
                    // embassy_time::Timer::after(Duration::from_nanos(10)).await;
                } else {
                info!("AUDIOTASK: Available Bytes: {}", dma_available_bytes);
                    // info!("Writing to the DMA");
                    chunk_end_index = min(
                        frame_size_bytes / 2,
                        dma_available_bytes / 2 + chunk_start_index,
                    );
                    info!("AUDIOTASK: startIDX:{}", chunk_start_index);
                    info!("AUDIOTASK: endIDX:{}", chunk_end_index);
                    // info!(
                    //     "AUDIOTASK: Writing:{}",
                    //     samples_to_write[chunk_start_index..chunk_end_index]
                    // );
                    transfer
                        .push(unsafe {
                            from_raw_parts(
                                samples_to_write[chunk_start_index..chunk_end_index]
                                    .as_ptr()
                                    .cast(),
                                (chunk_end_index - chunk_start_index) * 2,
                            )
                        })
                        .inspect_err(|e| info!("DMAError: {}", e))
                        .unwrap();
                    chunk_start_index = chunk_end_index;

                    if chunk_start_index >= frame_size_bytes / 2 {
                        break;
                    }
                }
            }
        }
    }
}
