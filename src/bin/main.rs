#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use alloc::string::String;
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::fat::Fat32Info;
use embedded_sdmmc::filesystem::ToShortFileName;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay as esp_delay;
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};
use esp_hal::i2c::master::{Config as I2CConfig, I2c};
use esp_hal::i2s::master::{Config as I2SConfig, I2s, Instance, UnitConfig};
use esp_hal::peripherals::{DMA_CH0, DMA_CH1, I2S0};
use esp_hal::spi::master::{Config as SPIConfig, Spi, SpiDma, SpiDmaBus};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{Blocking, assign_resources, dma, dma_descriptors, i2c};
use esp_println::println;
use log::info;

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
use tlv320dac3100::typedefs::VolumeControl;
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
    dac: DACResource<'d>{
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
type TLVType = TLV320DAC3100<I2c<'static, Blocking>>;
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
    dac_peripherals: TLVType,
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
        info!("{:?}", sdcard.num_bytes().unwrap());
        info!("{:?}", sdcard.get_card_type().unwrap());
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
    fn audio_init(r: DACResource<'static>) -> TLVType {
        let mut rst_pin = Output::new(r.dac_rst, Level::Low, OutputConfig::default());
        rst_pin.set_low();
        esp_delay::default().delay_micros(100);
        rst_pin.set_high();
        let i2c_bus_instance = i2c::master::I2c::new(r.i2c_module, I2CConfig::default())
            .unwrap()
            .with_scl(r.i2c_scl)
            .with_sda(r.i2c_sda);
        let mut dacObj = TLV320DAC3100::new(i2c_bus_instance);
        dacObj
            .set_dac_volume_control(false, false, VolumeControl::IndependentChannels)
            .unwrap();
        // dacObj.set_beep_sin_x(1).unwrap();
        (r,dacObj)
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
        info!("Listing {:?}", dir_input);
        dir_input
            .iterate_dir_lfn(&mut buf, |entry, buf| {
                info!("{:?}", entry.attributes);
                if entry.attributes.is_directory() {
                    // iter_dir();
                }
                if let Some(buf) = buf {
                    info!(" {:?}", buf);
                } else {
                    info!(".");
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
async fn audio_task(dac_peripherals: TLVType) {
    static audio_filename: &str = "stereo.flac";
    static FLAC_AUDIO: &[u8] = include_bytes!("../../assets/stereo.flac");
    let decoder = codec::codec::Decoder::new(audio_filename, FLAC_AUDIO);
    let pcm_samples = decoder.get_pcm_samples(FLAC_AUDIO);
    let i2s_driver = I2s::new(
        dac_peripherals.i2s_module,
        dac_peripherals.i2s_dma,
        I2SConfig::default(),
    )
    .unwrap();
    let (rx_descriptors, tx_descriptors) = dma_descriptors!(32000, 32000);
    let mut i2s_tx_writer = i2s_driver
        .i2s_tx
        .with_bclk(dac_peripherals.i2s_bclk)
        .with_dout(dac_peripherals.i2s_dout)
        .with_ws(dac_peripherals.i2s_ws)
        .build(tx_descriptors);
    i2s_tx_writer.apply_config(&UnitConfig::default()).unwrap();
    i2s_tx_writer.write_dma(&mut pcm_samples.samples());
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
    // generator version: 1.1.0

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let r = split_resources!(peripherals);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");
    let app_resource = AppResources::new(r);
    // TODO: Spawn some tasks
    let _ = spawner;
    // spawner.spawn(blink(r.led)).unwrap();
    // spawner
    //     .spawn(display_task(app_resource.display_object))
    //     .unwrap();
    // spawner
    //     .spawn(sdcard_task(app_resource.volume_manager))
    //     .unwrap();
    spawner
        .spawn(audio_task(app_resource.dac_peripherals))
        .unwrap();
}
