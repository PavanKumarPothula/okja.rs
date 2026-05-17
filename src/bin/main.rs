#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::slice::from_raw_parts;

use alloc::string::String;
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer, block_for};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::fat::Fat32Info;
use embedded_sdmmc::filesystem::ToShortFileName;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay as esp_delay;
use esp_hal::dma::ReadBuffer;
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};
use esp_hal::i2c::master::{Config as I2CConfig, I2c};
use esp_hal::i2s::master::{Channels, Config as I2SConfig, DataFormat, I2s, Instance, UnitConfig};
use esp_hal::peripherals::{DMA_CH0, DMA_CH1, GPIO17, GPIO18, GPIO43, GPIO44, I2S0};
use esp_hal::spi::master::{Config as SPIConfig, Spi, SpiDma, SpiDmaBus};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{
    Blocking, assign_resources, dma, dma_circular_buffers, dma_descriptors, dma_tx_buffer, i2c,
};
// use esp_println::{self as _, info};
use defmt::info;
use defmt_rtt as _;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use mipidsi::interface::{Generic8BitBus, ParallelInterface};
use mipidsi::options::{ColorOrder, Orientation};
use mipidsi::{Builder, models::ST7789};

// use mousefood::ratatui::Terminal;
// use mousefood::*;

use embedded_sdmmc::{
    Attributes, Block, BlockDevice, Directory, LfnBuffer, Mode as FileMode, RawFile, RawVolume,
    SdCard, ShortFileName, VolumeIdx, VolumeManager,
};
use okja::audio::codec;

use tlv320dac3100::TLV320DAC3100;
use tlv320dac3100::registers::CODEC_INTERFACE_CONTROL_1;
use tlv320dac3100::typedefs::*;

assign_resources! {
    Resources<'d>{
    led :LedResource<'d>{
        led_pin          : GPIO0,
    },
    display: DisplayResources<'d>{
        d0               : GPIO39 ,
        d1               : GPIO40 ,
        d2               : GPIO41 ,
        d3               : GPIO42 ,
        d4               : GPIO45 ,
        d5               : GPIO46 ,
        d6               : GPIO47 ,
        d7               : GPIO48 ,
        reset_pin        : GPIO5  ,
        chip_select      : GPIO6  ,
        data_command_pin : GPIO7  ,
        write_pin        : GPIO8  ,
        read_pin         : GPIO9  ,
        power_pin        : GPIO15 ,
        backlight_pin    : GPIO38 ,
    },
    sdcard: SDCardResources<'d>{
        spi_module       : SPI2,
        spi_sck          : GPIO2 ,
        spi_miso         : GPIO3 ,
        spi_mosi         : GPIO10,
        spi_cs           : GPIO11,
        dma              : DMA_CH0,
    },
    dac: DACPeripherals<'d>{
        i2c_module  : I2C0,
        i2c_scl     : GPIO16,
        i2c_sda     : GPIO21,
        i2s_module  : I2S0,
        i2s_dma     : DMA_CH1,
        i2s_dout    : GPIO17, // DATA - I2S data
        i2s_ws      : GPIO18,   // LRCLOCK - Word select
        i2s_bclk    : GPIO44, // BITCLOCK - I2S clock
        dac_rst     : GPIO43,  // DAC Reset Pin
    }
    }
}
struct DummyTimeSource;

impl embedded_sdmmc::TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        embedded_sdmmc::Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

// Init_done
struct DACResources {
    i2s_module: I2S0<'static>,
    i2s_dma: DMA_CH1<'static>,
    i2s_dout: GPIO17<'static>, // DATA - I2S data
    i2s_ws: GPIO18<'static>,   // LRCLOCK - Word select
    i2s_bclk: GPIO44<'static>, // BITCLOCK - I2S clock
    tlv_obj: TLV320DAC3100<I2c<'static, Blocking>>,
}

type VolumeManagerType = embedded_sdmmc::VolumeManager<
    SdCard<ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>, Delay>,
    DummyTimeSource,
    255,
    255,
    1,
>;
// type VolumeManagerType = VolumeManager<
//     SdCard<ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>, Delay>,
//     DummyTimeSource,
// >;
type DirectoryType<'a> = Directory<
    'a,
    SdCard<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>, Delay>,
    DummyTimeSource,
    255,
    255,
    1,
>;

type DisplayObjectType = mipidsi::Display<
    ParallelInterface<
        Generic8BitBus<
            Output<'static>,
            Output<'static>,
            Output<'static>,
            Output<'static>,
            Output<'static>,
            Output<'static>,
            Output<'static>,
            Output<'static>,
        >,
        Output<'static>,
        Output<'static>,
    >,
    ST7789,
    Output<'static>,
>;

struct AppResources {
    dac_peripherals: DACResources,
    volume_manager: VolumeManagerType,
    display_object: DisplayObjectType,
}
impl AppResources {
    #[allow(clippy::large_stack_frames)]
    fn new(r: Resources<'static>) -> Self {
        Self {
            dac_peripherals: Self::audio_init(r.dac),
            volume_manager: Self::sdcard_init(r.sdcard),
            display_object: Self::display_init(r.display),
        }
    }
    #[allow(clippy::large_stack_frames)]
    fn sdcard_init(r: SDCardResources<'static>) -> VolumeManagerType {
        let spi_interface = Spi::new(r.spi_module, SPIConfig::default())
            .unwrap()
            .with_sck(r.spi_sck)
            .with_mosi(r.spi_mosi)
            .with_miso(r.spi_miso);
        let spi_cs = Output::new(r.spi_cs, Level::High, OutputConfig::default());
        let spi_cell = ExclusiveDevice::new(spi_interface, spi_cs, Delay).unwrap();
        let sdcard = SdCard::new(spi_cell, Delay);
        // info!("{:?}", sdcard.num_bytes().unwrap());
        // info!("{:?}", sdcard.get_card_type().unwrap());
        VolumeManager::new_with_limits(sdcard, DummyTimeSource, 0)
    }

    fn display_init(r: DisplayResources<'static>) -> DisplayObjectType {
        let mut _pwr_pin = Output::new(r.power_pin, Level::High, OutputConfig::default());
        let mut _backlight = Output::new(r.backlight_pin, Level::High, OutputConfig::default());
        let mut _read = Output::new(r.read_pin, Level::High, OutputConfig::default());
        let mut _chip_sel = Output::new(r.chip_select, Level::Low, OutputConfig::default());

        let pin_bank = (
            Output::new(r.d0, Level::Low, OutputConfig::default()),
            Output::new(r.d1, Level::Low, OutputConfig::default()),
            Output::new(r.d2, Level::Low, OutputConfig::default()),
            Output::new(r.d3, Level::Low, OutputConfig::default()),
            Output::new(r.d4, Level::Low, OutputConfig::default()),
            Output::new(r.d5, Level::Low, OutputConfig::default()),
            Output::new(r.d6, Level::Low, OutputConfig::default()),
            Output::new(r.d7, Level::Low, OutputConfig::default()),
        );
        let display_interface = ParallelInterface::new(
            Generic8BitBus::new(pin_bank),
            Output::new(r.data_command_pin, Level::High, OutputConfig::default()),
            Output::new(r.write_pin, Level::High, OutputConfig::default()),
        );
        Builder::new(ST7789, display_interface)
            .reset_pin(Output::new(
                r.reset_pin,
                Level::High,
                OutputConfig::default(),
            ))
            .display_size(170, 320)
            .display_offset(35, 0)
            .color_order(ColorOrder::Rgb)
            .orientation(Orientation::default().rotate(mipidsi::options::Rotation::Deg90))
            .invert_colors(mipidsi::options::ColorInversion::Inverted)
            .init(&mut Delay)
            .unwrap()
    }
    fn audio_init(r: DACPeripherals<'static>) -> DACResources {
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
                SoftStepping::OneStepPerPeriod,
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
            .set_dac_left_volume_control(10.0)
            .expect("Error setting dac Left volume control");
        dac_obj
            .set_dac_right_volume_control(10.0)
            .expect("Error setting dac Right volume control");
        dac_obj
            .set_headphone_drivers(true, true, HpOutputVoltage::Common1_35V, false)
            .expect("Error setting dac Headphone drivers");

        dac_obj
            .set_hpl_driver(0, false)
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
            .set_class_d_spk_amp(false)
            .expect("Error setting class D spk amp");
        // dac_obj
        //     .set_class_d_spk_driver(OutputStage::Gain6dB, true)
        //     .expect("Error setting class D spk driver");
        // // dac_obj
        //     .set_left_analog_volume_to_spk(true, 0)
        //     .expect("Error setting left analog volume to spk");
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
        // dac_obj
        //     .set_int1_control_register(true, true, false, false, false, false)
        //     .expect("Error setting int1 control register");
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
}
#[embassy_executor::task]
async fn blink(r: LedResource<'static>) {
    let mut led = Output::new(r.led_pin, Level::Low, OutputConfig::default());
    loop {
        led.set_high();
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
        led.set_low();
        info!("Byeworld!");
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn display_task(mut display_object: DisplayObjectType) {
    // [TODO] Use ratatui to create UIs
    // let backend = EmbeddedBackend::new(display_object, EmbeddedBackendConfig::default());
    // let terminal = Terminal::new(backend)?;
    loop {
        info!("RED");
        Timer::after(Duration::from_secs(1)).await;
        display_object.clear(Rgb565::BLUE).unwrap();
        info!("BLUE");
        Timer::after(Duration::from_secs(1)).await;
        display_object.clear(Rgb565::GREEN).unwrap();
        info!("GREEN");
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn sdcard_task(volume_manager: VolumeManagerType) {
    // volume_manager.read(RawFile::ne(&self, other), buffer)
    let volume_handle = volume_manager.open_volume(VolumeIdx(0)).unwrap();
    let root_dir = volume_handle.open_root_dir().unwrap();
    fn iter_dir(dir_input: DirectoryType) {
        let mut storage = [0; 512];
        let mut buf = LfnBuffer::new(&mut storage);
        // info!("Listing {:?}", dir_input);
        dir_input
            .iterate_dir_lfn(&mut buf, |entry, buf| {
                // info!("{:?}", entry.attributes);
                if entry.attributes.is_directory() {
                    // iter_dir();
                }
                if let Some(buf) = buf {
                    // info!(" {:?}", buf);
                } else {
                    // info!(".");
                }
            })
            .unwrap();
    }

    iter_dir(root_dir.open_dir("/").unwrap());
    // fn iter_dir(root_dir: Directory<'_, SdCard<ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>, Delay>, DummyTimeSource, 4, 4, 1>)
    //     dir_input
    //         .open_dir(ShortFileName::create_from_str(&dir_input).unwrap())
    //         .unwrap()
    //         .iterate_dir_lfn(&mut buf, |file_name, buf| {
    //             info!("{:?}", file_name.attributes);
    //             if file_name.attributes.is_directory() {
    //                 iter_dir(file_name);
    //             }
    //             if let Some(buf) = buf {
    //                 info!(" {:?}", buf);
    //             } else {
    //                 info!(".");
    //             }
    //         })
    //         .unwrap();
    // };

    let flac_file = root_dir
        .open_file_in_dir(
            "Clipse, Pusha T, Malice - Inglorious Bastards.flac",
            FileMode::ReadOnly,
        )
        .unwrap();
}

#[embassy_executor::task]
async fn audio_task(dac_peripherals: DACResources) {
    info!("AUDIOTASK: Audio Started");
    static AUDIO_FILENAME: &str = "stereo.flac";
    static FLAC_AUDIO: &[u8] = include_bytes!("../../assets/stereo.flac");
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

    let mut decoder = codec::codec::Decoder::new(AUDIO_FILENAME, FLAC_AUDIO);

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
    let mut index = 0;
    for pair in dma_tx_buf.chunks_mut(2) {
        [pair[0], pair[1]] = SINE_WAVE[index % 96].to_ne_bytes();
        index += 1;
    }
    match decoder {
        codec::codec::Decoder::FLAC(ref this_meta) => {
            pos = this_meta.metadata.metadata_size;
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
extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();
#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // generator version: 1.3.0
    // generator parameters: --chip esp32s3 -o unstable-hal -o alloc -o embassy -o defmt -o stack-smashing-protection -o probe-rs -o panic-rtt-target -o embedded-test -o esp

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    // esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram); // Set as heap

    let r = split_resources!(peripherals);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");
    let app_resource = AppResources::new(r);
    // TODO: Spawn some tasks
    // let _ = spawner;
    // spawner.spawn(blink(r.led)).unwrap();
    // spawner
    //     .spawn(display_task(app_resource.display_object))
    //     .unwrap();
    // spawner
    //     .spawn(sdcard_task(app_resource.volume_manager))
    //     .unwrap();
    spawner.spawn(audio_task(app_resource.dac_peripherals).unwrap());
}
