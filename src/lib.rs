#![no_std]
pub mod audio;

use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::peripherals::{DMA_CH1, GPIO17, GPIO18, GPIO44, I2S0};
use esp_hal::spi::master::{Config as SPIConfig, Spi};
use esp_hal::{
    Blocking, assign_resources,
};
// use esp_println::{self as _, info};
use defmt_rtt as _;

use mipidsi::interface::{Generic8BitBus, ParallelInterface};
use mipidsi::options::{ColorOrder, Orientation};
use mipidsi::{Builder, models::ST7789};

// use mousefood::ratatui::Terminal;
// use mousefood::*;

use embedded_sdmmc::{
    Directory,
    SdCard, VolumeManager,
};

use tlv320dac3100::TLV320DAC3100;

assign_resources! {
    pub Resources<'d>{
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
pub struct DummyTimeSource;

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
pub struct DACResources {
    i2s_module: I2S0<'static>,
    i2s_dma: DMA_CH1<'static>,
    i2s_dout: GPIO17<'static>, // DATA - I2S data
    i2s_ws: GPIO18<'static>,   // LRCLOCK - Word select
    i2s_bclk: GPIO44<'static>, // BITCLOCK - I2S clock
    tlv_obj: TLV320DAC3100<I2c<'static, Blocking>>,
}

pub type VolumeManagerType = embedded_sdmmc::VolumeManager<
    SdCard<ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>, Delay>,
    DummyTimeSource,
    255,
    255,
    1,
>;
// pub type VolumeManagerType = VolumeManager<
//     SdCard<ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>, Delay>,
//     DummyTimeSource,
// >;
pub type DirectoryType<'a> = Directory<
    'a,
    SdCard<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>, Delay>,
    DummyTimeSource,
    255,
    255,
    1,
>;

pub type DisplayObjectType = mipidsi::Display<
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

pub struct AppResources {
    pub dac_peripherals: DACResources,
    pub volume_manager: VolumeManagerType,
    pub display_object: DisplayObjectType,
}
impl AppResources {
    #[allow(clippy::large_stack_frames)]
    pub fn new(r: Resources<'static>) -> Self {
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
        audio::init(r)
    }
}
