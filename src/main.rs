// CHIP-8 emulator
// references:
// - https://multigesture.net/articles/how-to-write-an-emulator-chip-8-interpreter/
// - https://en.wikipedia.org/wiki/CHIP-8
// - http://devernay.free.fr/hacks/chip8/C8TECH10.HTM

use rand::Rng;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use std::convert::TryFrom;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::time::Duration;
use std::collections::HashMap;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

mod utils;

// global constant
const VIDEO_SCALING: usize = 10;
const GFX_WIDTH: usize = 64;
const GFX_HEIGHT: usize = 32;
const PROGRAM_START_ADDRESS: usize = 0x200;

struct Machine {
    // main memory (4K)
    memory: [u8; 4096],
    registers: [u16; 16],
    index_register: u16,
    pc: usize,

    // graphics
    gfx: [u8; GFX_WIDTH * GFX_HEIGHT],
    // timers
    delay_timer: u16,
    sound_timer: u16,
    // stack
    stack: Vec<usize>,

    // current opcode
    opcode: u16,
    // program size
    program_size: usize,

    // current keys press state
    keys: HashMap<u16, u8>,

    // draw flag
    draw_flag: bool,
}

enum Timer {
    Sound,
    Delay,
}

type Register = usize;

// NNN: address
// NN: 8-bit constant
// N: 4-bit constant
// X and Y: 4-bit register identifier
// I : 16bit register (For memory address) (Similar to void pointer)
// VN: One of the 16 available variables. N may be 0 to F (hexadecimal)
// In an addition operation, VF is the carry flag, while in subtraction, it is the "no borrow" flag.
// In the draw instruction VF is set upon pixel collision.
// The address register, which is named I, is 16 bits wide and is used with several opcodes that involve memory operations.
#[derive(Debug)]
enum OpCode {
    Clear,                           // 00E0: Clears the screen
    Return,                          // 00EE: Returns from a subroutine
    JumpTo(u16),                     // 1NNN: Jumps to address NNN
    Call(u16),                       // 2NNN: Calls subroutine at NNN
    SkipEq(Register, u16), // 3XNN: Skips the next instruction if VX equals NN. (Usually the next instruction is a jump to skip a code block)
    SkipNotEq(Register, u16), // 4XNN: Skips the next instruction if VX doesn't equal NN. (Usually the next instruction is a jump to skip a code block)
    SkipEqXY(Register, Register), // 5XY0: Skips the next instruction if VX equals VY. (Usually the next instruction is a jump to skip a code block)
    SetX(Register, u16),          // 6XNN: Sets VX to NN
    AddX(Register, u16),          // 7XNN: Adds NN to VX. (Carry flag is not changed)
    AssignXY(Register, Register), // 8XY0: Sets VX to the value of VY
    OrXY(Register, Register),     // 8XY1: Vx = Vx | Vy
    AndXY(Register, Register),    // 8XY2: Vx = Vx & Vy
    XorXY(Register, Register),    // 8XY3: Vx = Vx ^ Vy
    AddXY(Register, Register), // 8XY4: Vx += Vy (VF is set to 1 when there's a carry, and to 0 when there isn't)
    SubXY(Register, Register), // 8XY5: Vx -= Vy (VF is set to 0 when there's a borrow, and 1 when there isn't)
    ShiftRightX1(Register), // 8XY6: Vx >> = 1 (Stores the least significant bit of VX in VF and then shifts VX to the right by 1)
    SubYX(Register, Register), // 8XY7: Vx = Vy - Vx (Sets VX to VY minus VX. VF is set to 0 when there's a borrow, and 1 when there isn't)
    ShiftLeftX1(Register), // 8XYE: Vx << = 1 (Stores the most significant bit of VX in VF and then shifts VX to the left by 1)
    SkipNotEqXY(Register, Register), // 9XY0: Skips the next instruction if VX doesn't equal VY. (Usually the next instruction is a jump to skip a code block)
    SetIR(u16),                      // ANNN: Sets I to the address NNN
    Flow(u16),                       // BNNN: PC = V0 + NNN (Jumps to the address NNN plus V0)
    RandX(Register, u16), // CXNN: Vx = rand() & NN (Sets VX to the result of a bitwise and operation on a random number (Typically: 0 to 255) and NN)
    Draw(Register, Register, u16), // DXYN: Draws a sprite at coordinate (Vx, Vy) that has a width of 8 pixels and a height of N+1 pixels. Each row of 8 pixels is read as bit-coded starting from memory location I
    KeyPressedX(Register), // EX9E: if(key() == Vx) Skips the next instruction if the key stored in VX is pressed. (Usually the next instruction is a jump to skip a code block)
    KeyNotPressedX(Register), // EXA1: if(key() != Vx) Skips the next instruction if the key stored in VX isn't pressed. (Usually the next instruction is a jump to skip a code block)
    TimerX(Register),         // FX07: Vx = get_delay()
    KeyPressX(Register),      // FX0A: Vx = get_key()
    SetDelayTimer(Register),  // FX15: delay_timer(Vx) Sets the delay timer to VX
    SetSoundTimer(Register),  // FX18: sound_timer(Vx) Sets the sound timer to VX
    MemAdd(Register),         // FX1E: I += Vx Adds VX to I. VF is not affected
    SpriteX(Register), // FX29: I = sprite_addr[Vx] (Sets I to the location of the sprite for the character in VX. Characters 0-F (in hexadecimal) are represented by a 4x5 font)
    BCD(Register),     // FX33: set_BCD(Vx)
    DumpX(Register),   // FX55: Stores V0 to VX (including VX) in memory starting at address I
    LoadX(Register), // FX65: Fills V0 to VX (including VX) with values from memory starting at address I. The offset from I is increased by 1 for each value written, but I itself is left unmodified
    Invalid,
}

fn extract_x(opcode: u16) -> Register {
    usize::from((opcode & 0x0F00) >> 8)
}
fn extract_y(opcode: u16) -> Register {
    usize::from((opcode & 0x00F0) >> 4)
}

fn parse_opcode(op: Option<u16>) -> OpCode {
    println!("parse_opcode: op = {:?}", op);
    if op == None {
        return OpCode::Invalid;
    }

    let opcode = op.unwrap();
    if opcode == 0x00E0 {
        return OpCode::Clear;
    }
    if opcode == 0x00EE {
        return OpCode::Return;
    }

    let class = (opcode & 0xF000) >> 12;
    let selector = opcode & 0x000F;

    match (class, selector) {
        (1, _) => OpCode::JumpTo(opcode & 0x0FFF),
        (2, _) => OpCode::Call(opcode & 0x0FFF),
        (3, _) => OpCode::SkipEq(extract_x(opcode), opcode & 0x00FF),
        (4, _) => OpCode::SkipNotEq(extract_x(opcode), opcode & 0x00FF),
        (5, 0) => OpCode::SkipEqXY(extract_x(opcode), extract_y(opcode)),
        (6, _) => OpCode::SetX(extract_x(opcode), opcode & 0x00FF),
        (7, _) => OpCode::AddX(extract_x(opcode), opcode & 0x00FF),
        (8, 0) => OpCode::AssignXY(extract_x(opcode), extract_y(opcode)),
        (8, 1) => OpCode::OrXY(extract_x(opcode), extract_y(opcode)),
        (8, 2) => OpCode::AndXY(extract_x(opcode), extract_y(opcode)),
        (8, 3) => OpCode::XorXY(extract_x(opcode), extract_y(opcode)),
        (8, 4) => OpCode::AddXY(extract_x(opcode), extract_y(opcode)),
        (8, 5) => OpCode::SubXY(extract_x(opcode), extract_y(opcode)),
        (8, 6) => OpCode::ShiftRightX1(extract_x(opcode)),
        (8, 7) => OpCode::SubYX(extract_x(opcode), extract_y(opcode)),
        (8, 0xE) => OpCode::ShiftLeftX1(extract_x(opcode)),
        (9, 0) => OpCode::SkipNotEqXY(extract_x(opcode), extract_y(opcode)),
        (0xA, _) => OpCode::SetIR(opcode & 0x0FFF),
        (0xB, _) => OpCode::Flow(opcode & 0x0FFF),
        (0xC, _) => OpCode::RandX(extract_x(opcode), opcode & 0x00FF),
        (0xD, _) => OpCode::Draw(extract_x(opcode), extract_y(opcode), opcode & 0x000F),
        (0xE, 9) => OpCode::KeyPressedX(extract_x(opcode)),
        (0xE, 1) => OpCode::KeyNotPressedX(extract_x(opcode)),
        (0xF, _) => {
            let sub_group = (opcode & 0x00F0) >> 4;
            match (sub_group, selector) {
                (0, 7) => OpCode::TimerX(extract_x(opcode)),
                (0, 0xA) => OpCode::KeyPressX(extract_x(opcode)),
                (1, 5) => OpCode::SetDelayTimer(extract_x(opcode)),
                (1, 8) => OpCode::SetSoundTimer(extract_x(opcode)),
                (1, 0xE) => OpCode::MemAdd(extract_x(opcode)),
                (2, 9) => OpCode::SpriteX(extract_x(opcode)),
                (3, 3) => OpCode::BCD(extract_x(opcode)),
                (5, 5) => OpCode::DumpX(extract_x(opcode)),
                (6, 5) => OpCode::LoadX(extract_x(opcode)),
                _ => OpCode::Invalid,
            }
        }
        _ => OpCode::Invalid,
    }
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
            opcode: 0,
            program_size: 0,
            keys: HashMap::new(),
            draw_flag: false,
        };
    }

    fn init(&mut self) {
        // reset
        *self = Machine::new();

        // set the Program Counter
        self.pc = PROGRAM_START_ADDRESS;

        // load fontset
        self.load_fontset();
    }

    fn set_timer(&mut self, t: Timer, v: u16) {
        match t {
            Timer::Sound => self.sound_timer = v,
            Timer::Delay => self.delay_timer = v,
        }
    }
    fn get_timer(&self, t: Timer) -> u16 {
        match t {
            Timer::Sound => self.sound_timer,
            Timer::Delay => self.delay_timer,
        }
    }

    fn load_program_file(&mut self, file: &str) -> Result<(), io::Error> {
        let mut f = File::open(file)?;
        let mut buffer = Vec::new();
        // read the whole file
        f.read_to_end(&mut buffer)?;
        self.load_program(buffer);
        Ok(())
    }

    fn load_program(&mut self, p: Vec<u8>) {
        // program start at 0x200
        let mut i = 0;
        for d in p {
            self.memory[PROGRAM_START_ADDRESS + i] = d;
            i += 1;
        }
        self.program_size = i;
        println!("program_size= {}", self.program_size);
    }

    fn fetch_opcode(&mut self) -> Option<u16> {
        println!(
            "fetch_opcode: PC = {} *** {}",
            self.pc,
            PROGRAM_START_ADDRESS + self.program_size
        );
        if self.pc > PROGRAM_START_ADDRESS + self.program_size {
            return None;
        }
        self.opcode = u16::from(self.memory[self.pc]) << 8 | u16::from(self.memory[self.pc + 1]);
        Some(self.opcode)
    }

    fn set_key_state(&mut self, k: sdl2::keyboard::Keycode, state: u8) -> Option<u8>{
        match k {
            sdl2::keyboard::Keycode::Num0 => self.keys.insert(0, state),
            sdl2::keyboard::Keycode::Num1 => self.keys.insert(1, state),
            sdl2::keyboard::Keycode::Num2 => self.keys.insert(2, state),
            sdl2::keyboard::Keycode::Num3 => self.keys.insert(3, state),
            sdl2::keyboard::Keycode::Num4 => self.keys.insert(4, state),
            sdl2::keyboard::Keycode::Num5 => self.keys.insert(5, state),
            sdl2::keyboard::Keycode::Num6 => self.keys.insert(6, state),
            sdl2::keyboard::Keycode::Num7 => self.keys.insert(7, state),
            sdl2::keyboard::Keycode::Num8 => self.keys.insert(8, state),
            sdl2::keyboard::Keycode::Num9 => self.keys.insert(9, state),
            sdl2::keyboard::Keycode::A => self.keys.insert(10, state),
            sdl2::keyboard::Keycode::B => self.keys.insert(11, state),
            sdl2::keyboard::Keycode::C => self.keys.insert(12, state),
            sdl2::keyboard::Keycode::D => self.keys.insert(13, state),
            sdl2::keyboard::Keycode::E => self.keys.insert(14, state),
            sdl2::keyboard::Keycode::F => self.keys.insert(15, state),
            _ => None,
        }
    }

    fn exec_single(&mut self) -> bool {
        let opcode = parse_opcode(self.fetch_opcode());
        println!("OPCODE = {:?}", opcode);

        self.draw_flag = false;
        match opcode {
            OpCode::Invalid => return false,
            OpCode::Clear => {
                self.gfx = [0; GFX_HEIGHT * GFX_WIDTH];
                self.draw_flag = true;
                self.pc_inc();
            }
            OpCode::Return => {
                let v = self.stack.pop().unwrap();
                self.pc = usize::from(v);
                self.pc_inc();
            }
            OpCode::JumpTo(n) => {
                self.pc = usize::from(n);
            }
            OpCode::Call(n) => {
                self.stack.push(self.pc);
                self.pc = usize::from(n);
            }
            OpCode::SkipEq(r, n) => {
                if self.registers[r] == n {
                    self.pc_inc();
                }
                self.pc_inc();
            }
            OpCode::SkipNotEq(r, n) => {
                if self.registers[r] != n {
                    self.pc_inc();
                }
                self.pc_inc();
            }
            OpCode::SkipEqXY(rx, ry) => {
                if self.registers[rx] == self.registers[ry] {
                    self.pc_inc();
                }
                self.pc_inc();
            }
            OpCode::SetX(r, n) => {
                self.registers[r] = n;
                self.pc_inc();
            }
            OpCode::AddX(r, n) => {
                self.registers[r] = (self.registers[r] + n) & 0x00FF; // force cast to 8bit
                self.pc_inc();
            }
            OpCode::AssignXY(rx, ry) => {
                self.registers[rx] = self.registers[ry];
                self.pc_inc();
            }
            OpCode::OrXY(rx, ry) => {
                self.registers[rx] |= self.registers[ry];
                self.pc_inc();
            }
            OpCode::AndXY(rx, ry) => {
                self.registers[rx] &= self.registers[ry];
                self.pc_inc();
            }
            OpCode::XorXY(rx, ry) => {
                self.registers[rx] ^= self.registers[ry];
                self.pc_inc();
            }
            OpCode::AddXY(rx, ry) => {
                self.registers[rx] += self.registers[ry];
                if self.registers[rx] > 255 {
                    self.registers[0xF] = 1; // set carry flag
                } else {
                    self.registers[0xF] = 0; // unset carry flag
                }
                self.registers[rx] &= 0x00FF;
                self.pc_inc();
            }
            OpCode::SubXY(rx, ry) => {
                if self.registers[rx] >= self.registers[ry] {
                    self.registers[rx] -= self.registers[ry];
                    self.registers[0xF] = 1; // set borrow flag
                } else {
                    self.registers[rx] = 256 - (self.registers[ry] - self.registers[rx]);
                    self.registers[0xF] = 0; // unset borrow flag
                }
                self.pc_inc();
            }
            OpCode::ShiftRightX1(r) => {
                let v = self.registers[r];
                let b = v & 0x0001;
                self.registers[0xF] = b;
                self.registers[r] = (v >> 1) & 0x00FF;
                self.pc_inc();
            }
            OpCode::SubYX(rx, ry) => {
                if self.registers[ry] >= self.registers[rx] {
                    self.registers[rx] = self.registers[ry] - self.registers[rx];
                    self.registers[0xF] = 1; // set borrow flag
                } else {
                    self.registers[rx] = 0;
                    self.registers[0xF] = 0; // unset borrow flag
                }
                self.pc_inc();
            }
            OpCode::ShiftLeftX1(r) => {
                let v = self.registers[r];
                let b = v & 0x80; // take the first bit
                self.registers[0xF] = b;
                self.registers[r] = (v << 1) & 0x00FF;
                self.pc_inc();
            }
            OpCode::SkipNotEqXY(rx, ry) => {
                if self.registers[rx] != self.registers[ry] {
                    self.opcode += 1;
                }
                self.pc_inc();
            }
            OpCode::SetIR(n) => {
                self.index_register = n;
                self.pc_inc();
            }
            OpCode::Flow(n) => {
                self.pc = usize::from(self.registers[0] + n);
            }
            OpCode::RandX(r, n) => {
                let mut rng = rand::thread_rng();
                self.registers[r] = rng.gen::<u16>() & n;
                self.pc_inc();
            }
            OpCode::KeyPressedX(r) => {
                if let Some(v) = self.keys.get(&self.registers[r]) {
                    if *v > 0 {
                        self.pc_inc();
                    }
                }
                self.pc_inc();
            }
            OpCode::KeyNotPressedX(r) => {
                match self.keys.get(&self.registers[r]) {
                    Some(v) => if *v == 0 { self.pc_inc(); }
                    None => self.pc_inc()
                }
                self.pc_inc();
            }
            OpCode::KeyPressX(r) => {
                for (k, v) in self.keys.clone() {
                    if v > 0 {
                        self.registers[r] = k;
                        self.pc_inc();
                    }
                }
            }
            OpCode::TimerX(r) => {
                self.registers[r] = self.get_timer(Timer::Delay);
                self.pc_inc();
            }
            OpCode::SetDelayTimer(r) => {
                self.set_timer(Timer::Delay, self.registers[r]);
                self.pc_inc();
            }
            OpCode::SetSoundTimer(r) => {
                self.set_timer(Timer::Sound, self.registers[r]);
                self.pc_inc();
            }
            OpCode::MemAdd(r) => {
                self.index_register += self.registers[r];
                self.pc_inc();
            }
            OpCode::SpriteX(r) => {
                self.index_register = self.registers[r] * 5;
                self.pc_inc();
            }
            OpCode::DumpX(r) => {
                for i in 0..=r {
                    let location = usize::from(self.index_register) + i;
                    self.memory[location] = u8::try_from(self.registers[i] & 0x00FF).unwrap();
                }
                self.pc_inc();
            }
            OpCode::LoadX(r) => {
                for i in 0..=r {
                    let location = usize::from(self.index_register) + i;
                    self.registers[i] = u16::from(self.memory[location]);
                }
                self.pc_inc();
            }
            OpCode::Draw(rx, ry, n) => {
                let x = usize::from(self.registers[rx]);
                let y = usize::from(self.registers[ry]);

                self.draw_flag = true;
                self.registers[0xF] = 0;
                for h in 0..n {
                    let byte_row = self.memory[usize::from(self.index_register + h)];
                    let bits_row = utils::convert_to_bits(byte_row);

                    for k in 0..8 {
                        let curr_x = (x + k) % GFX_WIDTH;
                        let curr_y = (y + usize::from(h)) % GFX_HEIGHT;

                        let pos_video = curr_y * GFX_WIDTH + curr_x;
                        let pixel_video = self.gfx[pos_video];
                        if pixel_video == 1 && bits_row[k] == pixel_video {
                            self.registers[0xF] = 1
                        };
                        self.gfx[pos_video] ^= bits_row[k];
                    }
                }
                self.pc_inc();
            }
            OpCode::BCD(r) => {
                let ds = utils::convert_to_bcd(self.registers[r]);

                self.memory[usize::from(self.index_register)] = ds[0];
                self.memory[usize::from(self.index_register + 1)] = ds[1];
                self.memory[usize::from(self.index_register + 2)] = ds[2];

                self.pc_inc();
            }
        }
        true
    }

    fn pc_inc(&mut self) {
        let opcode_mem_size = 2;
        self.pc += opcode_mem_size;
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

fn render(canvas: &mut WindowCanvas, gfx: &[u8; GFX_HEIGHT * GFX_WIDTH]) {
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.set_draw_color(Color::RGB(255, 255, 255));

    let s = u32::try_from(VIDEO_SCALING).unwrap();

    for y in 0..GFX_HEIGHT {
        for x in 0..GFX_WIDTH {
            let p: usize = y * GFX_WIDTH + x;
            if gfx[p] > 0 {
                let px = i32::try_from(x * VIDEO_SCALING).unwrap();
                let py = i32::try_from(y * VIDEO_SCALING).unwrap();

                match canvas.fill_rect(Rect::new(px, py, s, s)) {
                    Ok(_) => {}
                    _ => break
                }
            }
        }
    }
    canvas.present();
}

fn main() -> io::Result<()> {
    println!("C H I P - 8 - Emulator engine");

    let mut m = Machine::new();
    // init
    m.init();

    let program_file: String = match std::env::args().nth(1) {
        None => String::from("./data/test_opcode.rom"),
        Some(s) => s,
    };

    // load program
    match m.load_program_file(&program_file) {
        Ok(_) => println!("program loaded!"),
        Err(e) => panic!("cannot load program file `{}`: {}", program_file, e),
    }

    // set video
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window(
            "CHIP 8",
            u32::try_from(GFX_WIDTH * VIDEO_SCALING).unwrap(),
            u32::try_from(GFX_HEIGHT * VIDEO_SCALING).unwrap(),
        )
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();

    'running: loop {
        let mut refresh_window = false;

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
                Event::KeyDown { keycode: Some(kcode), .. } => {
                    m.set_key_state(kcode, 1);
                }
                Event::KeyUp { keycode: Some(kcode), .. } => {
                    m.set_key_state(kcode, 0);
                }
                Event::Window {..} => {
                    refresh_window = true;
                }
                _ => {}
            }
        }

        let alive = m.exec_single();
        if !alive {
            break 'running;
        }

        // Render
        if refresh_window || (alive && m.draw_flag) {
            render(&mut canvas, &m.gfx);
        }

        // timer
        if m.delay_timer > 0 {
            m.delay_timer -= 1;
        }
        if m.sound_timer > 0 {
            if m.sound_timer == 1 {
                println!("BEEP");
            }
            m.sound_timer -= 1;
        }

        // Time management!
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 120));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_fetch_opcode() {
        let mut m = Machine::new();
        // init
        m.init();
        m.load_program(vec![0xA2, 0xF0]);

        assert_eq!(0xA2F0, m.fetch_opcode().unwrap());
    }

    #[test]
    fn machine_fetch_simple_exec() {
        let mut m = Machine::new();
        // init
        m.init();

        // v0 = 5 + 2
        m.load_program(vec![
            0x70, 0x05, // V0 = 5
            0x71, 0x02, // V1 = 2
            0x80, 0x14, // V0 += V1
        ]);

        while m.exec_single() {}

        assert_eq!(7, m.registers[0]);
    }
}
