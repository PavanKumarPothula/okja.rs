#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_backtrace as _;
use esp_hal::assign_resources;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::timer::timg::TimerGroup;
use log::info;
use mipidsi::interface::{Generic8BitBus, ParallelInterface};
use mipidsi::options::{ColorOrder, Orientation, RefreshOrder};
use mipidsi::{Builder, models::ST7789};
assign_resources! {
    Resources<'d>{
    led :LedResource<'d>{
        led_pin: GPIO0,
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
// Init_done
#[embassy_executor::task]
async fn display_task(r: DisplayResources<'static>) {
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
    let display_bus = Generic8BitBus::new(pin_bank);
    let display_interface = ParallelInterface::new(
        display_bus,
        Output::new(r.data_command_pin, Level::High, OutputConfig::default()),
        Output::new(r.write_pin, Level::High, OutputConfig::default()),
    );
    let mut delay = embassy_time::Delay;
    let mut display_object = Builder::new(ST7789, display_interface)
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
        .init(&mut delay)
        .unwrap();
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
