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
                Err(_) => return 0, 
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

    fn get_input(&mut self ) -> u8
    {
        let result = self.serial_rx.read();
        match result {
            Ok(word) => return word,
            Err(_) => return 0,
        }
    }
}

#[entry]
fn main() -> ! {
    // start at (1,1) go to (8,8) first iteration
    // change to array of strings
    // 20x10 with wall, 9*19 actaul map
    let mut img :[&str;210]  =
    ["-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","\n",
    "|","*"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","|","\n",
    "|"," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," "," ","%","|","\n",
    "-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","-","\n"];
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
    scr.set_position(0, 0);
    scr.set_color_fgbg(Color::BLACK, false, Color::YELLOW, true);
    scr.send_str("Get Ready in 3!\n");
    sleep.delay_ms(1000_u32);
    scr.send_str("2\n");
    sleep.delay_ms(1000_u32);
    scr.send_str("1\n");
    sleep.delay_ms(1000_u32);
    scr.send_str("Start\n");
    sleep.delay_ms(1000_u32);
    scr.clear_screen();
    scr.set_position(0, 0);
    scr.set_cursor_visisble(true);
    scr.send_str("SNAKE game by Rufat & Oliver\n");
    scr.set_color_fgbg(Color::BLACK, false, Color::GREEN, true);
    for i in img.iter() {
        scr.send_str(i);
    }

    enum DirectionSnake {
        UP,
        DOWN,
        LEFT,
        RIGHT,
        STOPPED
    }
    let mut snake_direction = DirectionSnake::STOPPED;
    let mut snake_loc : [usize; 2] = [1, 1]; // 0th index row, 1st index column
    let mut snake_loc_arith : usize = 22;
    let mut user_score : u32 = 0;
    let mut dest_row : usize = 8;
    let mut dest_col : usize = 18;
    let mut dest_flattened : usize = dest_row * 21 + dest_col;
    loop { 
        // periodically check the RX buffer, and if you see 
        // any of W,A,S,D change the previous direction
        // if the current character is either - or |
        // print "You Scored %d"
        match rec.get_input() {
            b'w' => {
                snake_direction = DirectionSnake::UP;
                img[snake_loc_arith] = " ";
                snake_loc[0] -= 1;
                snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
                
            },
            b's' => {
                snake_direction = DirectionSnake::DOWN;
                img[snake_loc_arith] = " ";
                snake_loc[0] += 1;
                snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
            },
            b'a' => {
                snake_direction = DirectionSnake::LEFT;
                img[snake_loc_arith] = " ";
                snake_loc[1] -= 1;
                snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
            },
            b'd' => {
                snake_direction = DirectionSnake::RIGHT;
                img[snake_loc_arith] = " ";
                snake_loc[1] += 1;
                snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
            },
            _ => {
                match snake_direction {
                  DirectionSnake::UP => {
                    img[snake_loc_arith] = " ";
                    snake_loc[0] -= 1;
                    snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
                  }
                  DirectionSnake::DOWN => {
                    img[snake_loc_arith] = " ";
                    snake_loc[0] += 1;
                    snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
                  }
                  DirectionSnake::LEFT => {
                    img[snake_loc_arith] = " ";
                    snake_loc[1] -= 1;
                    snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
                  }
                  DirectionSnake::RIGHT => {
                    img[snake_loc_arith] = " ";
                    snake_loc[1] += 1;
                    snake_loc_arith = snake_loc[0] * 21 + snake_loc[1];
                  }
                  DirectionSnake::STOPPED => (),
                }
            },
        }
        if (snake_loc[0] == 0 || snake_loc[1] == 19 || snake_loc[1] == 0) {
            scr.set_color_fgbg(Color::BLACK, false, Color::RED, true);
            scr.send_str("You lost");
            loop {}
        }
        if (snake_loc_arith == dest_flattened) {
            user_score += 1;
            dest_row -= 1;
            dest_col -= 1;
            dest_flattened = dest_row * 21 + dest_col;
            img[dest_flattened] = "%";
        }
        img[snake_loc_arith] = "*";
        scr.clear_screen();
        scr.set_color_fgbg(Color::BLACK, false, Color::GREEN, true);
        scr.set_cursor_visisble(false);
        scr.set_position(0, 0);
        for i in img.iter() {
            scr.send_str(i);
        }
        sleep.delay_ms(500_u32);

    }
}
