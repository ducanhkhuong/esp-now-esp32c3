// #![no_std]
// #![no_main]

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
    gpio::{Input, Io, Pull},
    delay::Delay
};

use esp_println::println;

use esp_wifi::{
    esp_now::{PeerInfo, BROADCAST_ADDRESS},
    init, EspWifiInitFor,
};

struct ButtonHandler<'a> {
    button: Input<'a>,
    counter: i8,
}

impl<'a> ButtonHandler<'a> {
    fn new(button: Input<'a>) -> Self {
        ButtonHandler {
            button,
            counter: 0,
        }
    }

    
    fn check_press(&mut self, delay: &mut Delay) -> ButtonState {
        if self.button.is_low() {
            delay.delay_millis(10);
            if self.button.is_low() {
                self.counter += 1;
                while self.button.is_low() {}
            }
        }
        match self.counter {
            0 => ButtonState::Off,
            1 => ButtonState::Blue,
            2 => ButtonState::Green,
            3 => ButtonState::Red,
            _ => {
                self.counter = 0;
                ButtonState::Off
            }
        }
    }
}




#[derive(Debug)]
enum ButtonState {
    Off,
    Blue,
    Green,
    Red,
}


struct EspNowHandler<'a> {
    esp_now: esp_wifi::esp_now::EspNow<'a>,
}

impl<'a> EspNowHandler<'a> {
    fn new(init: &'a esp_wifi::EspWifiInitialization, wifi: impl esp_hal::peripheral::Peripheral<P = esp_hal::peripherals::WIFI> + 'a) -> Self {
        let esp_now = esp_wifi::esp_now::EspNow::new(init, wifi).unwrap();
        println!("esp-now version {:?}", esp_now.get_version().unwrap());
        EspNowHandler { esp_now }
    }

    async fn handle_communication(&mut self, btn_status: &[u8]) {
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
            let _status = self.esp_now.send_async(&r.info.src_address, btn_status).await;
        }
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
    let button = Input::new(io.pins.gpio3, Pull::Up);
    let mut delay = Delay::new();
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_alloc::heap_allocator!(72 * 1024);

    let init = init(
        EspWifiInitFor::Wifi,
        timg0.timer0,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    let wifi = peripherals.WIFI;
    let mut esp_now_handler = EspNowHandler::new(&init, wifi);

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timg1.timer0);

    let mut ticker = Ticker::every(Duration::from_millis(100));
    let mut button_handler = ButtonHandler::new(button);

    loop {
        let btn_state = button_handler.check_press(&mut delay);

        let btn_status = match btn_state {
            ButtonState::Off =>   b"0",
            ButtonState::Blue =>  b"1",
            ButtonState::Green => b"2",
            ButtonState::Red =>   b"3",
        };

        let res = select(ticker.next(), esp_now_handler.handle_communication(btn_status)).await;

        match res {
            Either::First(_) => {}
            Either::Second(_) => (),
        }
        
        println!("BUTTON [MASTER] ---> LED_RGB [SLAVE] : {:?}", btn_state);
    }
}
