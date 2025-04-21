#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Level, OutputConfig};
use esp_hal::spi::master::Config;
use esp_hal::time::Rate;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{clock::CpuClock, spi::master::Spi};
use rtt_target::{rprintln, rtt_init_print};

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    rprintln!("Panic: {:?}", panic_info);
    loop {}
}

extern crate alloc;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.3.1
    rtt_init_print!();
    rprintln!("Hello, world!");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    //let io = Io::new(peripherals.IO_MUX);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let _init = esp_wifi::init(
        timer1.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    // TODO: Spawn some tasks
    spawner.spawn(print_task()).ok();

    let mut mosi =
        esp_hal::gpio::Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
    mosi.set_low();
    Timer::after(Duration::from_millis(100)).await;

    let sck = peripherals.GPIO7;
    let mut spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_khz(2500)) // 1 / 2.5Mhz = 0.4us
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .unwrap()
    .with_mosi(mosi)
    .with_sck(sck);
    // 24bit GRB
    let mut brightness: u8 = 0; // 0-100

    let colors = [
        (0, 0, 0),
        (0, 255, 0),     // Red
        (255, 255, 255), // White
        (0, 0, 255),     // Blue
                         //(255, 0, 0), // Green
    ];
    loop {
        for (g, r, b) in colors.iter() {
            let mut green: u8 = *g;
            let mut red: u8 = *r;
            let mut blue: u8 = *b;

            brightness = brightness.wrapping_add(1);
            // This is here so I don't blind myself.
            if brightness >= 100 {
                brightness = 0;
            }

            let brightness_percent = brightness as f32 / 255.0;

            green = ((green as f32) * brightness_percent) as u8;
            red = ((red as f32) * brightness_percent) as u8;
            blue = ((blue as f32) * brightness_percent) as u8;
            // Datasheet claims its MSB for GRB but in reality its LSB BRG
            let color: u32 =
                blue as u32 | (red as u32).rotate_left(8) | (green as u32).rotate_left(16);
            rprintln!(
                "Color 0x{:#08X}, Brightness {:.2}%",
                color,
                brightness_percent * 100.0
            );
            let mut raw_data: u128 = 0;
            for (index, i) in (0..24usize).enumerate() {
                if ((color >> i) & 0x1) == 1 {
                    // 1 Code - 0.8us high, 0.45us low (+/- 150ns)
                    raw_data |= 0b110_u128 << (index * 3);
                    //data[index] = 0b110;
                } else {
                    // 0 code - 0.4us high, 0.85us low (+/- 150ns)
                    raw_data |= 0b100_u128 << (index * 3);
                    //data[index] = 0b100;
                }
            }
            let data: [u8; 16] = raw_data.to_be_bytes();
            let data: &[u8] = &data[7..16];
            rprintln!("Setting LED to GRB {:x?}", &data);
            spi.write(data).unwrap();
            Timer::after(Duration::from_millis(500)).await;
        }
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-beta.0/examples/src/bin
}

#[embassy_executor::task]
async fn print_task() {
    loop {
        //rprintln!("Hello, world!");
        Timer::after(Duration::from_millis(1000)).await;
    }
}
