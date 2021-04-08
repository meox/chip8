// CHIP-8 emulator

#[derive(Debug)]
struct Machine {
    // main memory (4K)
    memory: [u8; 4096],
    
    registers: [u8; 16],
    index_register: u16,
    pc: u32,

    // graphics
    gfx: [u8; 64*32],

    // timers
    delay_timer: u16,
    sound_timer: u16,

    // stack
    stack: Vec<u16>,
}

enum Timer{ Sound, Delay }

impl Machine {
    fn new() -> Machine {
        return Machine{
            index_register: 0,
            
            stack: Vec::new(),
        }
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
}

fn main() {
    println!("C H I P - 8");

}
