// CHIP-8 emulator
// references:
// - https://multigesture.net/articles/how-to-write-an-emulator-chip-8-interpreter/
// - https://en.wikipedia.org/wiki/CHIP-8
// - http://devernay.free.fr/hacks/chip8/C8TECH10.HTM

use std::io;
use std::io::prelude::*;
use std::fs::File;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::WindowCanvas;
use std::time::Duration;
use std::convert::TryFrom;

// global constant
const VIDEO_SCALING: u32 = 4;
const GFX_WIDTH: usize = 128;
const GFX_HEIGHT: usize = 32;

#[derive(Debug)]
struct Machine {
    // main memory (4K)
    memory: [u8; 4096],

    registers: [u8; 16],
    index_register: u16,
    pc: u32,

    // graphics
    gfx: [u8; GFX_WIDTH * GFX_HEIGHT],

    // timers
    delay_timer: u16,
    sound_timer: u16,

    // stack
    stack: Vec<u16>,
    sp: u16,

    // input key
    keys: [u8; 16],

    // current opcode
    opcode: u16,

    // program size
    program_size: u32,
}

enum Timer {
    Sound,
    Delay,
}

impl Machine {
    fn new() -> Machine {
        return Machine {
            memory: [0; 4096],
            registers: [0; 16],
            index_register: 0,
            pc: 0,
            gfx: [0; GFX_WIDTH * GFX_HEIGHT],
            delay_timer: u16::MAX,
            sound_timer: u16::MAX,
            stack: Vec::new(),
            sp: 0,
            keys: [0; 16],
            opcode: 0,
            program_size: 0,
        };
    }

    fn init(&mut self) {
        // reset
        *self = Machine::new();

        // set the Program Counter
        self.pc = 0x200;

        // load fontset
        self.load_fontset();
    }

    fn VS(self) -> u8 {
        self.registers[15]
    }

    fn set_timer(&mut self, t: Timer, v: u16) {
        match t {
            Timer::Sound => self.sound_timer = v,
            Timer::Delay => self.delay_timer = v,
        }
    }
    fn get_timer(self, t: Timer) -> u16 {
        match t {
            Timer::Sound => self.sound_timer,
            Timer::Delay => self.delay_timer,
        }
    }

    fn load_program(&mut self, file: &str) -> Result<(), io::Error> {
        let mut f = File::open(file)?;
        let mut buffer = Vec::new();
        // read the whole file
        let program_size = f.read_to_end(&mut buffer)?;
        self.program_size = u32::try_from(program_size).unwrap();

        // program start at 0x200
        let mut i = 0;
        for d in buffer {
            self.memory[0x200+i] = d;
            i += 1;
        }

        Ok(())
    }

    fn fetch_opcode(&mut self) {
        if self.pc > self.program_size {
            return
        }
    }

    fn load_fontset(&mut self) {
        let codes: [[u8; 5]; 16] = [
            [0xF0, 0x90, 0x90, 0x90, 0xF0], // 0
            [0x20, 0x60, 0x20, 0x20, 0x70], // 1
            [0xF0, 0x10, 0xF0, 0x80, 0xF0], // 2
            [0xF0, 0x10, 0xF0, 0x10, 0xF0], // 3
            [0x90, 0x90, 0xF0, 0x10, 0x10], // 4
            [0xF0, 0x80, 0xF0, 0x10, 0xF0], // 5
            [0xF0, 0x80, 0xF0, 0x90, 0xF0], // 6
            [0xF0, 0x10, 0x20, 0x40, 0x40], // 7
            [0xF0, 0x90, 0xF0, 0x90, 0xF0], // 8
            [0xF0, 0x90, 0xF0, 0x10, 0xF0], // 9
            [0xF0, 0x90, 0xF0, 0x90, 0x90], // A
            [0xE0, 0x90, 0xE0, 0x90, 0xE0], // B
            [0xF0, 0x80, 0x80, 0x80, 0xF0], // C
            [0xE0, 0x90, 0x90, 0x90, 0xE0], // D
            [0xF0, 0x80, 0xF0, 0x80, 0xF0], // E
            [0xF0, 0x80, 0xF0, 0x80, 0x80], // F
        ];

        let mut x = 0;
        for font_bytes in &codes {
            // copy the font to the memory
            for b in font_bytes {
                self.memory[x] = *b;
                x += 1;
            }
        }
    }
}

fn render(canvas: &mut WindowCanvas, color: Color) {
    canvas.set_draw_color(color);
    canvas.clear();
    canvas.present();
}

fn main() -> io::Result<()> {
    println!("C H I P - 8\nEmulator engine");

    let mut m = Machine::new();
    // init
    m.init();

    // load program
    let program_file = "pong";
    match m.load_program(program_file) {
        Ok(_) => println!("program loaded!"),
        Err(e) => panic!("cannot load program file `{}`: {}", program_file, e)
    }

    // set video

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("CHIP 8", 128 * VIDEO_SCALING, 32 * VIDEO_SCALING)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut i = 0;

    'running: loop {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                _ => {}
            }
        }

        // Update
        i = (i + 1) % 255;

        // Render
        render(&mut canvas, Color::RGB(i, 64, 255 - i));

        // Time management!
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
