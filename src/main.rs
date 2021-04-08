// CHIP-8 emulator
// references:
// - https://multigesture.net/articles/how-to-write-an-emulator-chip-8-interpreter/
// - https://en.wikipedia.org/wiki/CHIP-8
// - http://devernay.free.fr/hacks/chip8/C8TECH10.HTM

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::WindowCanvas;
use std::time::Duration;

#[derive(Debug)]
struct Machine {
    // main memory (4K)
    memory: [u8; 4096],

    registers: [u8; 16],
    index_register: u16,
    pc: u32,

    // graphics
    gfx: [u8; 64 * 32],

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
            gfx: [0; 64 * 32],
            delay_timer: u16::MAX,
            sound_timer: u16::MAX,
            stack: Vec::new(),
            sp: 0,
            keys: [0; 16],
            opcode: 0,
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

    fn load_program(&mut self, file: &str) {
        // program start at 0x200
    }

    fn load_fontset(&mut self) {
        let code_0: [u8; 5] = [0xF0, 0x90, 0x90, 0x90, 0xF0];
        let code_1: [u8; 5] = [0x20, 0x60, 0x20, 0x20, 0x70];
        let code_2: [u8; 5] = [0xF0, 0x10, 0xF0, 0x80, 0xF0];
        let code_3: [u8; 5] = [0xF0, 0x10, 0xF0, 0x10, 0xF0];
        let code_4: [u8; 5] = [0x90, 0x90, 0xF0, 0x10, 0x10];
        let code_5: [u8; 5] = [0xF0, 0x80, 0xF0, 0x10, 0xF0];
        let code_6: [u8; 5] = [0xF0, 0x80, 0xF0, 0x90, 0xF0];
        let code_7: [u8; 5] = [0xF0, 0x10, 0x20, 0x40, 0x40];
        let code_8: [u8; 5] = [0xF0, 0x90, 0xF0, 0x90, 0xF0];
        let code_9: [u8; 5] = [0xF0, 0x90, 0xF0, 0x10, 0xF0];
        let code_a: [u8; 5] = [0xF0, 0x90, 0xF0, 0x90, 0x90];
        let code_b: [u8; 5] = [0xE0, 0x90, 0xE0, 0x90, 0xE0];
        let code_c: [u8; 5] = [0xF0, 0x80, 0x80, 0x80, 0xF0];
        let code_d: [u8; 5] = [0xE0, 0x90, 0x90, 0x90, 0xE0];
        let code_e: [u8; 5] = [0xF0, 0x80, 0xF0, 0x80, 0xF0];
        let code_f: [u8; 5] = [0xF0, 0x80, 0xF0, 0x80, 0x80];
    }
}

fn render(canvas: &mut WindowCanvas, color: Color) {
    canvas.set_draw_color(color);
    canvas.clear();
    canvas.present();
}

fn main() {
    println!("C H I P - 8\nEmulator engine");

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("CHIP 8", 800, 600)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut m = Machine::new();
    // init
    m.init();

    // load program
    m.load_program("pong");

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
}
