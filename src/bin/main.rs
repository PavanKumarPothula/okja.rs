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
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::spi::master::{Config, Spi, SpiDma, SpiDmaBus};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{assign_resources, dma};

use log::info;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use mipidsi::interface::{Generic8BitBus, ParallelInterface};
use mipidsi::options::{ColorOrder, Orientation};
use mipidsi::{Builder, models::ST7789};

// use mousefood::ratatui::Terminal;
// use mousefood::*;

use embedded_sdmmc::{
    Attributes, BlockDevice, Directory, LfnBuffer, Mode as FileMode, RawVolume, SdCard,
    ShortFileName, VolumeIdx, VolumeManager,
};

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

type VolumeManagerType = VolumeManager<
    SdCard<ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>, Delay>,
    DummyTimeSource,
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
    volume_manager: VolumeManagerType,
    display_object: DisplayObjectType,
}
impl AppResources {
    #[allow(clippy::large_stack_frames)]
    fn new(r: Resources<'static>) -> Self {
        Self {
            volume_manager: Self::sdcard_init(r.sdcard),
            display_object: Self::display_init(r.display),
        }
    }
    #[allow(clippy::large_stack_frames)]
    fn sdcard_init(r: SDCardResources<'static>) -> VolumeManagerType {
        let spi_interface = Spi::new(r.spi_module, Config::default())
            .unwrap()
            .with_sck(r.spi_sck)
            .with_mosi(r.spi_mosi)
            .with_miso(r.spi_miso);
        let spi_cs = Output::new(r.spi_cs, Level::High, OutputConfig::default());
        let spi_cell = ExclusiveDevice::new(spi_interface, spi_cs, Delay).unwrap();
        let sdcard = SdCard::new(spi_cell, Delay);
        VolumeManager::new(sdcard, DummyTimeSource)
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
    let volume_handle = volume_manager.open_volume(VolumeIdx(0)).unwrap();
    let root_dir = volume_handle.open_root_dir().unwrap();
    let mut storage = [0; 512];
    let mut buf = LfnBuffer::new(&mut storage);
    fn iter_dir(dir_name: String) -> () {
        root_dir
            .open_dir(ShortFileName::create_from_str(&dir_name).unwrap())
            .unwrap()
            .iterate_dir_lfn(&mut buf, |file_name, buf| {
                info!("{:?}", file_name.attributes);
                if file_name.attributes.is_directory() {
                    iter_dir(file_name);
                }
                if let Some(buf) = buf {
                    info!(" {:?}", buf);
                } else {
                    info!(".");
                }
            })
            .unwrap();
    };
    iter_dir(root_dir);
    let flac_file = root_dir
        .open_file_in_dir(
            "Clipse, Pusha T, Malice - Inglorious Bastards.flac",
            FileMode::ReadOnly,
        )
        .unwrap();
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
    spawner
        .spawn(sdcard_task(app_resource.volume_manager))
        .unwrap();
}
