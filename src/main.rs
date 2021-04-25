#![no_std]
#![no_main]

extern crate panic_halt;

use riscv_rt::entry;
use hifive1::hal::prelude::*;
use hifive1::hal::serial::*;
use hifive1::hal::DeviceResources;
use hifive1::hal::e310x::UART0;
use hifive1::pin;
use hifive1::hal::delay::Sleep;
use core::convert::TryInto;
use core::cmp::max;

const LF : u8 =  10;
const CR: u8  =  13;
const CSI : &str = "\x1b["; 
//const CSI : &str = "["; 

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

/* Screen is a struct with methods to control the terminal display (e.g, writing text at 
    certain positions in different colors, etc). 
*/
struct Screen<'a>
{
    serial_tx: &'a mut Tx<UART0>,   
}

impl<'a> Screen<'a>
{
    /* Send byte over UART and wait until it is successfully received before returning.
     * Note: To send a char, you can cast it to a u8. For example: send_byte('x' as u8);
     * */
    fn send_byte(&mut self, byte: u8) -> ()
    {

        // If LF ('\n') is sent, add a CR beforehand. 
        if byte == LF {
            self.send_byte(CR);
        }
        loop {
            let result = self.serial_tx.write(byte);
            if result.is_ok() {
                break;
            }
        }
    }

    /* Send str over UART and wait until it is successfully received before returning */
    fn send_str(&mut self, s: &str) -> ()
    {
        for byte in s.bytes() {
            self.send_byte(byte);
        }
    }

    // Print an integer in range [-2147483647, -2147483647] by sending the
    // characters over UART.
    fn send_int(&mut self, x: i32) 
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
            self.send_byte(*c);
        }
    }

    // Make the cursor visible/invisible.
    fn set_cursor_visisble(&mut self, visible: bool)
    {
        self.send_str(CSI);
        if visible {
            self.send_str("?25h");
        }
        else {
            self.send_str("?25l");
        }
    }

    /* Set the cursor position to the given row/col index. 
     * The row and col arguments assume that (0, 0) is the top-left
     * character on the screen.
     */
    fn set_position(&mut self, row: i32, col: i32)
    {
        // Correct for the fact that ANSI expects (1, 1) to be the top-left. 
        // Also ensure that value is not negative.
        let row = max(row, 0) + 1; 
        let col = max(col, 0) + 1;
        self.send_str(CSI);
        self.send_int(row);
        self.send_str(";");
        self.send_int(col);
        self.send_str("H");
    }

    /* Set the text foreground and background color.
     */
    fn set_color_fgbg(&mut self, fg: Color, fg_bright: bool, bg: Color, bg_bright: bool)
    {
        self.send_str(CSI);
        self.send_int(fg.fg(fg_bright));
        self.send_str(";");
        self.send_int(bg.bg(bg_bright));
        self.send_str("m");
    }

    /* Reset color to normal
     */
    fn reset_color(&mut self)
    {
        self.send_str(CSI);
        self.send_str("m");
    }

    /* Clear screen
     */
    fn clear_screen(&mut self)
    {
        self.reset_color();
        self.send_str(CSI);
        self.send_str("2J");
    }
}

struct InputReceiver<'a>
{
    serial_rx: &'a mut Rx<UART0>,   
}

impl<'a> InputReceiver<'a>
{
    // waits until input is available, then returns the byte
    // received.
    fn wait_for_input(&mut self ) -> u8
    {
        loop {
            let result = self.serial_rx.read();
            match result {
                Ok(word) => return word,
                Err(_) => (), 
            }
        }
    }

    // Clears any characters in the hardware RX FIFO buffer, by
    // reading data until it's empty. 
    // Returns the last (most recent) character read from the buffer, if any.
    fn clear_buffer(&mut self ) -> Option<u8>
    {
        let mut opt: Option<u8> = None;
        loop {
            let result = self.serial_rx.read();
            if result.is_ok() {
                opt = result.ok(); // convert from Result to Option
            }
            else {
                break;
            }
        };
        return opt;
    }

    fn get_input(&mut self ) -> Result<u8, &str>
    {
        let result = self.serial_rx.read();
        match result {
            Ok(word) => Ok(word),
            Err(_) => Err("No input available"), 
        }
    }
}

#[entry]
fn main() -> ! {
    let dr = DeviceResources::take().unwrap();
    let p = dr.peripherals;
    let pins = dr.pins;

    // Configure clocks
    let clocks = hifive1::clock::configure(p.PRCI, p.AONCLK, 320.mhz().into());
    // get the local interrupts struct
    let clint = dr.core_peripherals.clint;
    // get the sleep struct
    let mut sleep = Sleep::new(clint.mtimecmp, clocks);

    let tx = pin!(pins, uart0_tx).into_iof0();
    let rx = pin!(pins, uart0_rx).into_iof0();
    
    // create new e310x_hal::serial::Serial struct to control the serial (UART) peripheral.
    let serial = Serial::new(p.UART0, (tx, rx), 115_200.bps(), clocks);
    // split the configured Serial struct into an Rx and a Tx struct
    let (mut serial_tx, mut serial_rx) = serial.split();

    let mut scr = Screen {serial_tx: &mut serial_tx};
    let mut rec = InputReceiver {serial_rx: &mut serial_rx};

    scr.clear_screen();
    sleep.delay_ms(5000_u32);

    rec.clear_buffer();
    scr.set_cursor_visisble(false);
    scr.set_position(4, 4);
    scr.set_color_fgbg(Color::BLACK, false, Color::YELLOW, true);
    
    scr.send_str("Press any key");
    rec.wait_for_input();

    scr.set_position(8, 1);
    scr.set_color_fgbg(Color::GREEN, true, Color::BLUE, false);
    scr.send_str("test");
    scr.send_byte('.' as u8);
    loop { }
}
