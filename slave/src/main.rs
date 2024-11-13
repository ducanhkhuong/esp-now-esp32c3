#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};
use esp_alloc as _;
use esp_backtrace as _;

use esp_hal::{
    prelude::*,
    rng::Rng, 
    timer::timg::TimerGroup,
    gpio::Io,
};

use esp_hal::gpio::Output;
use esp_println::println;
use esp_hal::gpio::Level;  

use esp_wifi::{
    esp_now::{PeerInfo, BROADCAST_ADDRESS},
    init, EspWifiInitFor,
};


#[derive(Debug, Clone, Copy)]
enum LedState {
    On,
    Off,
}

impl LedState {
    fn apply(self, led: &mut Output<'_>) {
        match self {
            LedState::On  => led.set_low(),
            LedState::Off => led.set_high(),
        }
    }
}

struct Device<'a> {
    esp_now: esp_wifi::esp_now::EspNow<'a>,
    led_1: Output<'a>,
    led_2: Output<'a>,
    led_3: Output<'a>,
}

impl<'a> Device<'a> {
    fn new(
        esp_now: esp_wifi::esp_now::EspNow<'a>,
        led_1: Output<'a>,
        led_2: Output<'a>,
        led_3: Output<'a>,
    ) -> Self {
        Self { esp_now, led_1, led_2, led_3 }
    }

    async fn handle_message(&mut self, data: &[u8]) {
        if let Some(&first_byte) = data.get(0) {
            let led_states = match first_byte {
                48 => (LedState::Off, LedState::Off, LedState::Off),
                49 => (LedState::On, LedState::Off, LedState::Off),
                50 => (LedState::Off, LedState::On, LedState::Off),
                51 => (LedState::Off, LedState::Off, LedState::On),
                _ =>  {
                    (LedState::Off, LedState::Off, LedState::Off)
                }
            };
            self.update_leds(led_states);
            println!("LED_RGB [SLAVE] : {:?}", led_states);
        }
    }

    fn update_leds(&mut self, states: (LedState, LedState, LedState)) {
        states.0.apply(&mut self.led_1);
        states.1.apply(&mut self.led_2);
        states.2.apply(&mut self.led_3);
    }

    async fn handle_communication(&mut self) {
        loop {
            let r = self.esp_now.receive_async().await;
            if r.info.dst_address == BROADCAST_ADDRESS {
                if !self.esp_now.peer_exists(&r.info.src_address) {
                    self.esp_now
                        .add_peer(PeerInfo {
                            peer_address: r.info.src_address,
                            lmk: None,
                            channel: None,
                            encrypt: false,
                        })
                        .unwrap();
                }
            }
            self.handle_message(&r.data).await;
        }
    }

    async fn send_message(&mut self) {
        let _status = self.esp_now.send_async(&BROADCAST_ADDRESS, b"0123456789").await;
    }
}






#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

     let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let led_1 = Output::new(io.pins.gpio4, Level::High);
    let led_2 = Output::new(io.pins.gpio3, Level::High);
    let led_3 = Output::new(io.pins.gpio1, Level::High);

    esp_alloc::heap_allocator!(72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let init = init(EspWifiInitFor::Wifi, timg0.timer0, Rng::new(peripherals.RNG), peripherals.RADIO_CLK).unwrap();
    let wifi = peripherals.WIFI;
    let esp_now = esp_wifi::esp_now::EspNow::new(&init, wifi).unwrap();

    println!("esp-now version {:?}", esp_now.get_version().unwrap());

    let mut device = Device::new(esp_now, led_1, led_2, led_3);
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timg1.timer0);

    let mut ticker = Ticker::every(Duration::from_millis(100));

    loop {
        let res = select(ticker.next(), device.handle_communication()).await;

        match res {
            Either::First(_) => {
                device.send_message().await;
            }
            Either::Second(_) => (),
        }
    }
}
