#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::timer::timg::TimerGroup;
// use esp_println::{self as _, info};
use defmt::info;
use defmt_rtt as _;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

// use mousefood::ratatui::Terminal;
// use mousefood::*;

use embedded_sdmmc::{LfnBuffer, Mode as FileMode, VolumeIdx};

use okja::audio::{AUDIO_DECODER, PLAY_PAUSE_STATE};
use okja::*;

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
                if let Some(_buf) = buf {
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

    let _flac_file = root_dir
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
    // generator version: 1.3.0
    // generator parameters: --chip esp32s3 -o unstable-hal -o alloc -o embassy -o defmt -o stack-smashing-protection -o probe-rs -o panic-rtt-target -o embedded-test -o esp

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram); // Set as heap

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
    spawner.spawn(okja::audio::player_task(app_resource.dac_peripherals).unwrap());

    Timer::after(Duration::from_secs(1)).await;
    static AUDIO_FILENAME: &str = "stereo.flac";
    static FLAC_AUDIO: &[u8] = include_bytes!("../../assets/stereo.flac");
    let file_info = audio::FileInfo {
        file_name: AUDIO_FILENAME,
        file_bytes: FLAC_AUDIO,
    };
    AUDIO_DECODER.reset();
    AUDIO_DECODER.signal(file_info);
    loop {
        PLAY_PAUSE_STATE.signal(audio::PlayPauseState::Play);
        Timer::after(Duration::from_secs(5)).await;
        PLAY_PAUSE_STATE.signal(audio::PlayPauseState::Pause);
        Timer::after(Duration::from_secs(1)).await;
    }
}
