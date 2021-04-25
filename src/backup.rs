#![no_std]
#![no_main]

extern crate panic_halt;

use riscv_rt::entry;
use hifive1::hal::prelude::*;
use hifive1::hal::delay::Sleep;
use hifive1::hal::serial::*;
use hifive1::hal::DeviceResources;
use hifive1::hal::e310x::UART0;
use hifive1::pin;
use core::convert::TryInto;
use core::cmp::max;

const LF : u8 =  10;
const CR: u8  =  13;
const CSI : &str = "\x1b["; 
//const CSI : &str = "^["; 

#[allow(dead_code)]
enum Color {
    BLACK, RED, GREEN, YELLOW, BLUE, MAGENTA, CYAN, WHITE,
}

impl Color 
{
    /* Return the number corresponding to the ANSI code to set the foreground to the given
     * 4-bit color.
     * If bright is true, use the bright version of that color.
     * See: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
     */
    fn fg(&self, bright : bool) -> i32 {
        let val = match *self {
            Color::BLACK => 30,
            Color::RED => 31,
            Color::GREEN => 32,
            Color::YELLOW => 33,
            Color::BLUE => 34,
            Color::MAGENTA => 35,
            Color::CYAN => 36,
            Color::WHITE => 37,
        };
        if bright {
            val + 60
        }
        else {
            val
        }
    }

    /* Return the number corresponding to the ANSI code to set the background to the given
     * 4-bit color.
     * If bright is true, use the bright version of that color.
     * See: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
     */
    fn bg(&self, bright: bool) -> i32 {
        self.fg(bright) + 10
    }
}

/* Set the cursor position to the given row/col index. 
 * The row and col arguments assume that (0, 0) is the top-left
 * character on the screen.
 */
fn set_position(row: i32, col: i32, serial_tx: &mut Tx<UART0>)
{
    // Correct for the fact that ANSI expects (1, 1) to be the top-left. 
    // Also ensure that value is not negative.
    let row = max(row, 0) + 1; 
    let col = max(col, 0) + 1;
    uart_send_str(CSI, serial_tx);
    uart_send_int(row, serial_tx);
    uart_send_str(";", serial_tx);
    uart_send_int(col, serial_tx);
    uart_send_str("H", serial_tx);
}

/* Set the text foreground and background color.
 */
fn set_color_fgbg(fg: Color, fg_bright: bool, bg: Color, bg_bright: bool, serial_tx: &mut Tx<UART0>)
{
    uart_send_str(CSI, serial_tx);
    uart_send_int(fg.fg(fg_bright), serial_tx);
    uart_send_str(";", serial_tx);
    uart_send_int(bg.bg(bg_bright), serial_tx);
    uart_send_str("m", serial_tx);
}

/* Reset color to normal
 */
fn reset_color(serial_tx: &mut Tx<UART0>)
{
    uart_send_str(CSI, serial_tx);
    uart_send_str("m", serial_tx);
}

/* Clear screen
 */
fn clear_screen(serial_tx: &mut Tx<UART0>)
{
    reset_color(serial_tx);
    uart_send_str(CSI, serial_tx);
    uart_send_str("2J", serial_tx);
}

// Make the cursor visible/invisible.
fn set_cursor_visisble(visible: bool, serial_tx: &mut Tx<UART0>)
{
    uart_send_str(CSI, serial_tx);
    if visible {
        uart_send_str("?25h", serial_tx);
    }
    else {
        uart_send_str("?25l", serial_tx);
    }
}

/* Send byte and wait until it is successfully received before returning.
 * If LF ('\n') is sent, a CR will be added beforehand. 
 * */
fn uart_send_byte(byte: u8, serial_tx: &mut Tx<UART0>) -> ()
{

    if byte == LF {
        uart_send_byte(CR, serial_tx);
    }
    loop {
        let result = serial_tx.write(byte);
        if result.is_ok() {
            break;
        }
    }
}

/* Send str and wait until it is successfully received before returning */
fn uart_send_str(s: &str, serial_tx: &mut Tx<UART0>) -> ()
{
    for byte in s.as_bytes().iter() {
        uart_send_byte(*byte, serial_tx);
    }
}

// Print an integer in range [-2147483647, -2147483647].
fn uart_send_int(x: i32, serial_tx: &mut Tx<UART0>) 
{
    let positive: bool;
    let mut abs_x : u32;
    let mut buf = [0; 11]; //buffer to hold characters in reverse order
    let mut i = 0; //index into buffer
    if x < 0 {
        positive = false;
        abs_x = (-x).try_into().unwrap();
    }
    else {
        positive = true;
        abs_x = x.try_into().unwrap();
    }
    loop {
        buf[i] = ((abs_x % 10) + '0' as u32) as u8;
        abs_x = abs_x / 10;
        i = i + 1;
        if abs_x == 0 {
            break;
        }
    }
    if !positive {
        buf[i] = '-' as u8;
        i = i + 1;
    }
    // buf[0..i] now holds the bytes we need to send in reverse order
    for c in buf[0..i].iter().rev() {
        uart_send_byte(*c, serial_tx);
    }
}

#[entry]
fn main() -> ! {
    let dr = DeviceResources::take().unwrap();
    let p = dr.peripherals;
    let pins = dr.pins;

    // Configure clocks
    let clocks = hifive1::clock::configure(p.PRCI, p.AONCLK, 320.mhz().into());

    let tx = pin!(pins, uart0_tx).into_iof0();
    let rx = pin!(pins, uart0_rx).into_iof0();
    let serial = Serial::new(p.UART0, (tx, rx), 115_200.bps(), clocks);
    let (mut serial_tx, mut serial_rx) = serial.split();

    // get the local interrupts struct
    let clint = dr.core_peripherals.clint;

    // get the sleep struct
    let mut sleep = Sleep::new(clint.mtimecmp, clocks);
    const PERIOD: u32 = 200; // .2s

    clear_screen(&mut serial_tx);
    set_cursor_visisble(false, &mut serial_tx);
    set_position(4, 4, &mut serial_tx);
    set_color_fgbg(Color::BLACK, false, Color::YELLOW, true, &mut serial_tx);
    uart_send_str("Press any key", &mut serial_tx);
    loop {
        let result = serial_rx.read();
        match result {
            Ok(_word) => break,
            Err(_) => (), 
        };
        //sleep 
        sleep.delay_ms(PERIOD);
    };
    set_position(8, 1, &mut serial_tx);
    set_color_fgbg(Color::GREEN, true, Color::BLUE, false, &mut serial_tx);
    uart_send_str("test", &mut serial_tx);
    loop {}
}

struct Screen<'a>
{
    serial_tx: &'a mut Tx<UART0>,   
}

impl<'a> Screen<'a>
{

}