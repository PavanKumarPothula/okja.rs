#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_backtrace as _;
use esp_hal::assign_resources;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};
use esp_hal::peripherals::{Peripherals, SPI2};
use esp_hal::spi::master::{Config, Spi};
use esp_hal::timer::timg::TimerGroup;
use log::info;
use mipidsi::Display;
use mipidsi::interface::{Generic8BitBus, Generic16BitBus, OutputBus, ParallelInterface};
use mipidsi::options::Orientation;
use mipidsi::{Builder, models::ST7789};
assign_resources! {
    Resources<'d>{
    led :LedResource<'d>{
        led_pin: GPIO0,
    },
    display: DisplayResources<'d>{
        spi_cs: GPIO
        d0:GPIO39 ,
        d1:GPIO40 ,
        d2:GPIO41 ,
        d3:GPIO42 ,
        d4:GPIO45 ,
        d5:GPIO46 ,
        d6:GPIO47 ,
        d7:GPIO48 ,
        res:GPIO5  ,
        cs:GPIO6  ,
        dc:GPIO7  ,
        wr:GPIO8  ,
        rd:GPIO9  ,
        pwr:GPIO15,
        bl: GPIO38,
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
#[embassy_executor::task]
async fn display_task(r: DisplayResources<'static>) {
    let mut pwr_pin = Output::new(r.pwr, Level::High, OutputConfig::default());
    pwr_pin.set_high();
    let mut backlight = Output::new(r.bl, Level::High, OutputConfig::default());
    backlight.set_high();
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
    // let display_bus = Generic8BitBus::new(pin_bank);
    // let display_interface = ParallelInterface::new(
    //     display_bus,
    //     Output::new(r.dc, Level::High, OutputConfig::default()),
    //     Output::new(r.wr, Level::High, OutputConfig::default()),
    // );
    let display_bus = Spi::new(SPI2, Config::default())?
        .with_sck(GPIO12)
        .with_mosi(GPIO11)
        .with_miso(GPIO13);
    let mut display_object = Builder::new(ST7789, display_interface)
        .reset_pin(Output::new(r.res, Level::High, OutputConfig::default()))
        .display_size(170, 320)
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .init(&mut Delay)
        .unwrap();
    loop {
        display_object.clear(Rgb565::RED).unwrap();
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

    // TODO: Spawn some tasks
    let _ = spawner;
    // spawner.spawn(blink(r.led)).unwrap();
    spawner.spawn(display_task(r.display)).unwrap();
}
