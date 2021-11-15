#![no_main]
#![no_std]

use rtt_target::{rprintln};
use stm32l4xx_hal;
use rtic::app;
use cortex_m_rt::{exception, ExceptionFrame};
use core::panic::PanicInfo;
use core::sync::atomic;
use core::sync::atomic::Ordering;

pub const CLOCKS_FREQ_HZ: u32 = 80_000_000; //80 MHz
pub type UsrLed = stm32l4xx_hal::gpio::gpioa::PA5<stm32l4xx_hal::gpio::Output<stm32l4xx_hal::gpio::PushPull>>;

#[app(device = stm32l4xx_hal::stm32, peripherals = true, dispatchers = [TSC, FLASH])]
mod app {
    use dwt_systick_monotonic::DwtSystick;
    use rtt_target::rtt_init_print;
    use crate::{rprintln, CLOCKS_FREQ_HZ};
    use stm32l4xx_hal::{
        gpio::{GpioExt},
        pac::{USART2},
        serial::{self, Config, Serial},
        prelude::*
    };
    use bno08x_rvc;
    use cortex_m::asm;
    use bbqueue::BBBuffer;
    use core::borrow::Borrow;

    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<{ CLOCKS_FREQ_HZ }>; // 1000 Hz / 1 ms granularity

    #[local]
    struct Local {
        usr_led: crate::UsrLed,
        proc: bno08x_rvc::processor::Processor,
        rx: serial::Rx<USART2>,
    }
    #[shared]
    struct Shared {
        pars: bno08x_rvc::parser::Parser,
    }


    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {

        static BB: BBBuffer<{ bno08x_rvc::BUFFER_SIZE }> = BBBuffer::new();
        rtt_init_print!();
        rprintln!("Initializing... ");

        let (proc, pars) = match bno08x_rvc::create(BB.borrow()) {
            Ok((proc, pars)) => (proc, pars),
            Err(e) => {
                panic!("Can't create bno08x-rvc : {:?}", e);
            }
        };

        let dev = cx.device;
        let mut flash = dev.FLASH.constrain();
        let cp = cx.core;
        let rcc_reg = dev.RCC;
        rcc_reg.apb1enr1.modify(|_, w| w.can1en().set_bit());

        let mut rcc = rcc_reg.constrain();

        let mut gpioa = dev.GPIOA.split(&mut rcc.ahb2);
        let mut pwr = dev.PWR.constrain(&mut rcc.apb1r1);

        let clocks = rcc
            .cfgr
            .sysclk(CLOCKS_FREQ_HZ.hz())
            .pclk1(CLOCKS_FREQ_HZ.hz())
            .pclk2(CLOCKS_FREQ_HZ.hz())
            .freeze(&mut flash.acr, &mut pwr);

        let usr_led = gpioa.pa5.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

        let tx_pin = gpioa.pa2.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
        let rx_pin = gpioa.pa3.into_af7(&mut gpioa.moder, &mut gpioa.afrl);

        let mut serial = Serial::usart2(
            dev.USART2,
            (tx_pin, rx_pin),
            Config::default().baudrate(bno08x_rvc::BNO08X_UART_RVC_BAUD_RATE.bps()),
            clocks,
            &mut rcc.apb1r1,
        );
        serial.listen(serial::Event::Rxne);

        let (_tx, rx) = serial.split();

        let systick = cp.SYST;
        let mut dcb =  cp.DCB;
        let  dwt =  cp.DWT;

        let mono = DwtSystick::new(&mut dcb, dwt, systick, clocks.sysclk().0);
        rprintln!("done.");

        (
            Shared { pars },
            Local {
                usr_led,
                proc,
                rx
            },
            init::Monotonics(mono)
        )
    }

    #[idle(shared = [pars])]
    fn idle(mut cx: idle::Context) -> !{
        loop{
            cx.shared.pars.lock(|p| {
                rprintln!("Get last Frame: {:?}", p.get_last_raw_frame());

            });
            asm::delay(80_000_000);
        }
    }

    #[task(shared = [pars], priority = 4)]
    fn parse(mut cx: parse::Context) {
        cx.shared.pars.lock(|p| {
            match p.worker(|frame|{
                rprintln!("Rx Frame: {:?}", frame);
            }){
                Ok(_) => { }
                Err(_e) => { }
            };
        })
    }

    #[task(binds = USART2, local = [proc, rx], priority = 5)]
    fn usart2(cx: usart2::Context) {
        let rx: &mut serial::Rx<USART2> = cx.local.rx;
        let proc = cx.local.proc;

        match rx.read() {
            Ok(b) => {
                match proc.process_slice(&[b]) {
                    Ok(_) => {  parse::spawn(); }
                    Err(_e) => {  }
                }
            },
            Err(_e) => { }
        };
    }
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}

#[inline(never)]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    rprintln!("Panic {:?}", _info);
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
}