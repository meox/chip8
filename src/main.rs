// CHIP-8 emulator
// references:
// - https://multigesture.net/articles/how-to-write-an-emulator-chip-8-interpreter/
// - https://en.wikipedia.org/wiki/CHIP-8
// - http://devernay.free.fr/hacks/chip8/C8TECH10.HTM

use rand::Rng;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::WindowCanvas;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::time::Duration;

// global constant
const VIDEO_SCALING: u32 = 4;
const GFX_WIDTH: usize = 128;
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
    sp: usize,
    // input key
    keys: [u8; 16],
    // current opcode
    opcode: u16,
    // program size
    program_size: usize,

    // current key press
    key_pressed: Option<u16>,
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

fn extractX(opcode: u16) -> Register {
    usize::from((opcode & 0x0F00) >> 8)
}
fn extractY(opcode: u16) -> Register {
    usize::from((opcode & 0x00F0) >> 4)
}

fn parse_opcode(opcode: u16) -> OpCode {
    if opcode == 0x00E0 {
        return OpCode::Clear;
    }
    if opcode == 0x00EE {
        return OpCode::Return;
    }

    let class = opcode & 0xF000;
    let selector = opcode & 0x000F;
    match (class, selector) {
        (1, _) => OpCode::JumpTo(opcode & 0x0FFF),
        (2, _) => OpCode::Call(opcode & 0x0FFF),
        (3, _) => OpCode::SkipEq(extractX(opcode), opcode & 0x00FF),
        (4, _) => OpCode::SkipNotEq(extractX(opcode), opcode & 0x00FF),
        (5, 0) => OpCode::SkipEqXY(extractX(opcode), extractY(opcode)),
        (6, _) => OpCode::SetX(extractX(opcode), opcode & 0x00FF),
        (7, _) => OpCode::AddX(extractX(opcode), opcode & 0x00FF),
        (8, 0) => OpCode::AssignXY(extractX(opcode), extractY(opcode)),
        (8, 1) => OpCode::OrXY(extractX(opcode), extractY(opcode)),
        (8, 2) => OpCode::AndXY(extractX(opcode), extractY(opcode)),
        (8, 3) => OpCode::XorXY(extractX(opcode), extractY(opcode)),
        (8, 4) => OpCode::AddXY(extractX(opcode), extractY(opcode)),
        (8, 5) => OpCode::SubXY(extractX(opcode), extractY(opcode)),
        (8, 6) => OpCode::ShiftRightX1(extractX(opcode)),
        (8, 7) => OpCode::SubYX(extractX(opcode), extractY(opcode)),
        (8, 0xE) => OpCode::ShiftLeftX1(extractX(opcode)),
        (9, 0) => OpCode::SkipNotEqXY(extractX(opcode), extractY(opcode)),
        (0xA, _) => OpCode::SetIR(opcode & 0x0FFF),
        (0xB, _) => OpCode::Flow(opcode & 0x0FFF),
        (0xC, _) => OpCode::RandX(extractX(opcode), opcode & 0x00FF),
        (0xD, _) => OpCode::Draw(extractX(opcode), extractY(opcode), opcode & 0x000F),
        (0xE, 9) => OpCode::KeyPressedX(extractX(opcode)),
        (0xE, 1) => OpCode::KeyNotPressedX(extractX(opcode)),
        (0xF, _) => {
            let z = opcode & 0x00F0;
            match (z, selector) {
                (0, 7) => OpCode::TimerX(extractX(opcode)),
                (0, 0xA) => OpCode::KeyPressX(extractX(opcode)),
                (1, 5) => OpCode::SetDelayTimer(extractX(opcode)),
                (1, 8) => OpCode::SetSoundTimer(extractX(opcode)),
                (1, 0xE) => OpCode::MemAdd(extractX(opcode)),
                (2, 9) => OpCode::SpriteX(extractX(opcode)),
                (3, 3) => OpCode::BCD(extractX(opcode)),
                (5, 5) => OpCode::DumpX(extractX(opcode)),
                (6, 5) => OpCode::LoadX(extractX(opcode)),
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
            sp: 0,
            keys: [0; 16],
            opcode: 0,
            program_size: 0,
            key_pressed: None,
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

    fn VS(self) -> u16 {
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

    fn load_program_file(&mut self, file: &str) -> Result<(), io::Error> {
        let mut f = File::open(file)?;
        let mut buffer = Vec::new();
        // read the whole file
        let program_size = f.read_to_end(&mut buffer)?;
        self.program_size = program_size;

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
    }

    fn fetch_opcode(&mut self) -> u16 {
        if self.pc > PROGRAM_START_ADDRESS + self.program_size {
            return 0;
        }
        self.opcode = u16::from(self.memory[self.pc]) << 8 | u16::from(self.memory[self.pc + 1]);
        self.pc += 2;
        return self.opcode;
    }

    fn set_key_pressed(&mut self, k: Option<sdl2::keyboard::Keycode>) {
        match k {
            Some(sdl2::keyboard::Keycode::Num0) => self.key_pressed = Some(0),
            Some(sdl2::keyboard::Keycode::Num1) => self.key_pressed = Some(1),
            Some(sdl2::keyboard::Keycode::Num2) => self.key_pressed = Some(2),
            Some(sdl2::keyboard::Keycode::Num3) => self.key_pressed = Some(3),
            Some(sdl2::keyboard::Keycode::Num4) => self.key_pressed = Some(4),
            Some(sdl2::keyboard::Keycode::Num5) => self.key_pressed = Some(5),
            Some(sdl2::keyboard::Keycode::Num6) => self.key_pressed = Some(6),
            Some(sdl2::keyboard::Keycode::Num7) => self.key_pressed = Some(7),
            Some(sdl2::keyboard::Keycode::Num8) => self.key_pressed = Some(8),
            Some(sdl2::keyboard::Keycode::Num9) => self.key_pressed = Some(9),
            Some(sdl2::keyboard::Keycode::A) => self.key_pressed = Some(10),
            Some(sdl2::keyboard::Keycode::B) => self.key_pressed = Some(11),
            Some(sdl2::keyboard::Keycode::C) => self.key_pressed = Some(12),
            Some(sdl2::keyboard::Keycode::D) => self.key_pressed = Some(13),
            Some(sdl2::keyboard::Keycode::E) => self.key_pressed = Some(14),
            Some(sdl2::keyboard::Keycode::F) => self.key_pressed = Some(15),
            _ => self.key_pressed = None,
        }
    }

    fn exec(&mut self) -> bool {
        let opcode = parse_opcode(self.fetch_opcode());
        match opcode {
            OpCode::Clear => self.gfx = [0; GFX_HEIGHT * GFX_WIDTH],
            OpCode::Return => {
                let v = self.stack[self.sp];
                self.pc = usize::from(v);
                self.sp -= 1;
            }
            OpCode::JumpTo(n) => self.pc = usize::from(n),
            OpCode::Call(n) => {
                self.stack[self.sp] = self.pc;
                self.sp += 1;
                self.pc = usize::from(n);
            }
            OpCode::SkipEq(r, n) => {
                if self.registers[r] == n {
                    self.pc += 1;
                }
                self.pc += 1;
            }
            OpCode::SkipNotEq(r, n) => {
                if self.registers[r] != n {
                    self.pc += 1;
                }
                self.pc += 1;
            }
            OpCode::SkipEqXY(rx, ry) => {
                if self.registers[rx] == self.registers[ry] {
                    self.pc += 1;
                }
                self.pc += 1;
            }
            OpCode::SetX(r, n) => {
                self.registers[r] = n;
                self.pc += 1;
            }
            OpCode::AddX(r, n) => {
                self.registers[r] = (self.registers[r] + n) & 0x00FF; // force cast to 8bit
                self.pc += 1;
            }
            OpCode::AssignXY(rx, ry) => {
                self.registers[rx] = self.registers[ry];
                self.pc += 1;
            }
            OpCode::OrXY(rx, ry) => {
                self.registers[rx] |= self.registers[ry];
                self.pc += 1;
            }
            OpCode::AndXY(rx, ry) => {
                self.registers[rx] &= self.registers[ry];
                self.pc += 1;
            }
            OpCode::XorXY(rx, ry) => {
                self.registers[rx] ^= self.registers[ry];
                self.pc += 1;
            }
            OpCode::AddXY(rx, ry) => {
                self.registers[rx] += self.registers[ry];
                if self.registers[rx] > 255 {
                    self.registers[0xF] = 1; // set carry flag
                } else {
                    self.registers[0xF] = 0; // unset carry flag
                }
                self.registers[rx] &= 0x00FF;
                self.pc += 1;
            }
            OpCode::SubXY(rx, ry) => {
                if self.registers[rx] >= self.registers[ry] {
                    self.registers[rx] -= self.registers[ry];
                    self.registers[0xF] = 1; // set borrow flag
                } else {
                    self.registers[rx] = 0;
                    self.registers[0xF] = 0; // unset borrow flag
                }
                self.pc += 1;
            }
            OpCode::ShiftRightX1(r) => {
                let b = self.registers[r] % 2;
                self.registers[0xF] = b;
                self.registers[r] >>= 1;
                self.pc += 1;
            }
            OpCode::SubYX(rx, ry) => {
                if self.registers[ry] >= self.registers[rx] {
                    self.registers[rx] = self.registers[ry] - self.registers[rx];
                    self.registers[0xF] = 1; // set borrow flag
                } else {
                    self.registers[rx] = 0;
                    self.registers[0xF] = 0; // unset borrow flag
                }
                self.pc += 1;
            }
            OpCode::ShiftLeftX1(r) => {
                let b = self.registers[r] & 0x80; // take the first bit
                self.registers[0xF] = b;
                self.registers[r] <<= 1;
                self.pc += 1;
            }
            OpCode::SkipNotEqXY(rx, ry) => {
                if self.registers[rx] != self.registers[ry] {
                    self.opcode += 1;
                }
                self.pc += 1;
            }
            OpCode::SetIR(n) => self.index_register = n,
            OpCode::Flow(n) => self.pc = usize::from(self.registers[0] + n),
            OpCode::RandX(r, n) => {
                let mut rng = rand::thread_rng();
                self.registers[r] = rng.gen::<u16>() & n;
                self.pc += 1;
            }
            OpCode::KeyPressedX(r) => {
                let k = usize::from(self.registers[r]);
                if self.keys[k] > 0 {
                    // if pressed
                    self.pc += 1;
                }
                self.pc += 1;
            }
            OpCode::KeyNotPressedX(r) => {
                let k = usize::from(self.registers[r]);
                if self.keys[k] == 0 {
                    // if not pressed
                    self.pc += 1
                }
                self.pc += 1
            }
            OpCode::KeyPressX(r) => {
                if let Some(k) = self.key_pressed {
                    self.registers[r] = k;
                    self.pc += 1;
                }
            }
            _ => println!("Not implemented: {:?}", opcode),
        }
        true
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
    println!("C H I P - 8 - Emulator engine");

    let mut m = Machine::new();
    // init
    m.init();

    // load program
    let program_file = "data/test_opcode.ch8";
    match m.load_program_file(program_file) {
        Ok(_) => println!("program loaded!"),
        Err(e) => panic!("cannot load program file `{}`: {}", program_file, e),
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
                Event::KeyDown { keycode: kcode, .. } => m.set_key_pressed(kcode),
                Event::KeyUp { .. } => m.set_key_pressed(None),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_fetch_opcode() {
        let mut m = Machine::new();
        // init
        m.init();
        m.load_program(vec![0xA2, 0xF0]);

        assert_eq!(0xA2F0, m.fetch_opcode());
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
        m.exec();

        assert_eq!(7, m.registers[0]);
    }
}
