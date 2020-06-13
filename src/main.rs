use std::fs::File;
use std::io::Read;

const TEXTURE_SIZE :usize = 32*64;
const SCREEN_WIDTH :u8 = 64;
const SCREEN_HEIGHT :u8 = 32;

const FONT_SET : [u8;80] =
[
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80  // F
];


fn was_key_pressed() -> bool
{
    return false;
}

fn get_first_pressed_key() -> u8
{
    return 0;
}

struct OppCodeData
{
    nnn : u16,
    nn : u8,
    n : u8,
    x : u8,
    y : u8,
}

impl OppCodeData
{
    fn new(oppcode: u16) -> OppCodeData
    {
        let nnn_ : u16 = oppcode & 0x0FFF;
        let nn_ : u8 = (oppcode & 0x00FF) as u8;
        let n_ : u8 = (oppcode & 0x000F) as u8;
        let x_ : u8 = ((oppcode & 0x0F00) >> 8) as u8;
        let y_ : u8 = ((oppcode & 0x00F0) >> 4) as u8; 
        return OppCodeData{
            nnn: nnn_,
            nn: nn_,
            n: n_,
            x: x_,
            y: y_,
        };
    }

    fn init(&mut self, oppcode: u16) -> ()
    {
        self.nnn = oppcode & 0x0FFF;
        self.nn= (oppcode & 0x00FF) as u8;
        self.n= (oppcode & 0x000F) as u8;
        self.x= ((oppcode & 0x0F00) >> 8) as u8;
        self.y= ((oppcode & 0x00F0) >> 4) as u8; 
    }

}

struct Chip
{
    current_opcode : u16,
    memory : [u8;4096],
    registers : [u8;16],
    index_register : u16,
    program_counter : u16,
    texture : [u8; 64*32],
    delay_timer : u8,
    sound_timer : u8,
    stack : [u16;16],
    stack_pointer : u16,
    keys : [u8; 16],
    oppcode_data: OppCodeData,
}

impl Chip
{
    fn new() -> Chip
    {
        let mut chip = Chip{
            current_opcode: 0,
            memory: [0;4096],
            registers: [0;16],
            index_register: 0,
            program_counter: 0x200,
            texture: [0;TEXTURE_SIZE],
            delay_timer: 0,
            sound_timer: 0,
            stack: [0;16],
            stack_pointer: 0,
            keys : [0;16],
            oppcode_data: OppCodeData::new(0x0000),
        };
        chip.load_font(&FONT_SET);
        return chip;
    }

    fn load_rom(&mut self, file_location: &str) -> ()
    {
        let path : &std::path::Path = std::path::Path::new(file_location);
        
        let mut file = match File::open(&path)
        {
            Err(why) => panic!("Could not load rom {}: {}",path.display(),why),
            Ok(file) => file,
        };
        let file_size : u64 = file.metadata().unwrap().len();
        let mut buffer: [u8;4096] = [0;4096]; // rom can't be bigger than main memory
        file.read(&mut buffer).unwrap();

        for i in 0..file_size
        {
            self.memory[(i + 0x200) as usize] = buffer[i as usize];
        }
    }

    fn emulate_cycle(&mut self) -> ()
    {
        // Fetch opcode
        let opcode_lhs : u16 = (self.memory[self.program_counter as usize] as u16) << 8;
        let opcode_rhs : u16 = self.memory[(self.program_counter+1) as usize] as u16;
        self.current_opcode = opcode_lhs | opcode_rhs;
        self.oppcode_data.init(self.current_opcode);

        // Decode and execute opcode
        match self.current_opcode & 0xF000
        {
            // Clear or return opcodes use least significant byte
            0x0000 => match self.current_opcode & 0x000F
            {
                // 0x00E0: Clears the screen
                0x0000 => self.clear_screen(),
            
                // 0x0EE: returns from subroutine
                0x000E => self.return_from_subroutine(),
            
                _ => panic!("Unknown opcode: {}", self.current_opcode), 
            },
            // 0x1NNN jumps to address NNN
            0x1000 => self.jump_to_address(),
            
            // 0x2NNN call subroutine
            0x2000 => self.call_subroutine(),
            
            // 0x3XNN skip if x equal
            0x3000 => self.skip_if_x_equal(),

            // 0x4XNN skip if x not equal
            0x4000 => self.skip_if_x_not_equal(),

            // 0x5XY0 skip if x and y are equal
            0x5000 => self.skip_if_x_y_equal(),

            // 0x6XNN set register x to nn
            0x6000 => self.assign_nn(),

            // 0x7XNN add nn to register x
            0x7000 => self.add_nnn(),

            // 0x8___
            0x8000 => match self.current_opcode & 0x000F
            {
                // 0x8XY0 set register x to register y
                0x0000 => self.assign(),

                // 0x8XY1 set x to x or y
                0x0001 => self.or(),

                // 0x8XY2 set x to x and y
                0x0002 => self.and(),

                // 0x8XY3 set x to x xor y
                0x003 => self.xor(),
                
                // 0x8XY4 set x to x + y and set register F to 0 if there is no carry and 1 if there is
                0x004 => self.add(),

                // 0x8XY5 set x to x-y and set register F to 0 if there is no borrow and 1 if there is
                0x005 => self.subtract_y_from_x(),

                // 0x8XY6 store the least significant bit in register F and then
                // shift x to the right by one
                0x006 => self.shift_x_right(),

                // 0x8XY7 set x to y - x and set register F to 0 if there is no borrow and 1 if there is
                0x007 => self.subtract_x_from_y(),

                // 0x8XYE store the most significant bit in register F and then
                // shift x to the left by one 
                0x00E => self.shift_x_left(),

                _  => panic!("Unknown opcode: {}", self.current_opcode),
            }

            // 0x9XY0 skips the next instruction if register x == register y
            0x900 => self.skip_if_equals(),

            // 0xANNN sets I (index_register) to the address NNN
            0xA000 => self.set_index_register(),

            // 0xBNNN jumps to the address NNN + register 0
            0xB000 => self.jump_to_address_plus_register_0(),

            // 0xCXNN sets register x to rnd() & NN
            0xC000 => self.set_x_to_random_and(),

            // 0xDXYN draws a sprite at coordinate (register x, register y).
            // Sprite is 8xN in size and sprite memory is read from location I (index_register)
            // register 0xF is set to 1 if any pixels are flipped (collision) and to 0 else.
            0xD000 => self.draw_sprite(),

            0xE000 => match self.current_opcode & 0x000F
            {
                // 0xEX9E skips the next instruction if the key stored in register x is pressed.
                0x000E => self.skip_if_key_is_pressed(),

                // 0xEXA1 skips the next instruction if the key stored in register x is not pressed.
                0x0001 => self.skip_if_key_is_not_pressed(),

                _ => panic!("Unknown opcode: {}", self.current_opcode),
            }
            
            0xF000 => match self.current_opcode & 0x00FF
            {
                // 0xFX07 stores the delay timer to register x.
                0x0007 => self.get_delay_timer(),

                // 0xFX0A waits for key press and stores it in register x.
                0x000A => self.wait_for_key_press(),

                // 0xFX15 sets the delay timer to register x.
                0x0015 => self.set_delay_timer(),

                // 0xFX18 sets the sound timer to register x.
                0x0018 => self.set_sound_timer(),

                // 0xFX1E adds register x to the index register (I) and sets register 0xF to 1 in case of overflow.
                0x001E => self.add_to_index(),
                
                // 0xFX29 sets the index register to the memory location of the sprite of the 
                // character stored in register x.
                0x0029 => self.set_sprite_address(),

                // 0xFX33 i don't know what this does
                0x0033 => self.binary_coded_decimal(),

                // 0xFX55 dump registers 0 - x into main memory.
                0x0055 => self.register_dump(),

                // 0xFX65 load main memory into registers 0 - x.
                0x0065 => self.register_load(),

                _ => panic!("Unknown opcode: {}", self.current_opcode),
            }

            _ => panic!("Unknown opcode: {}", self.current_opcode),
        }
        // Advance program counter
        self.program_counter += 2;

        // Update timers
        if self.delay_timer > 0
        {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0
        {
            if self.sound_timer == 1
            {
                println!("BEEP!");
            }
            self.sound_timer -= 1;
        }
    }

    fn load_font(&mut self, font_set: &[u8;80]) -> ()
    {
        self.memory[..80].copy_from_slice(font_set);
    }

    // Opcode implementations

    /// 0x00E0: Clears the screen.
    fn clear_screen(&mut self) -> ()
    {
        self.texture = [0;TEXTURE_SIZE];
    }

    /// 0x00EE: Returns from subroutine.
    fn return_from_subroutine(&mut self) -> ()
    {
        // stack pop
        self.stack_pointer -= 1; 
        self.program_counter = self.stack[self.stack_pointer as usize];
    }

    /// 0x1NNN: Jumps to the given address.
    fn jump_to_address(&mut self) -> ()
    {
        self.program_counter = self.oppcode_data.nnn;
    }

    /// 0x2NNN: Calls the given subroutine.
    fn call_subroutine(&mut self) -> ()
    {
        // stack push
        self.stack[self.stack_pointer as usize] = self.program_counter;
        self.stack_pointer += 1;
        
        self.program_counter = self.oppcode_data.nnn;
    }

    /// 0x3XNN: Skips the next instruction if register x equals NN.
    fn skip_if_x_equal(&mut self) -> ()
    {
        if self.registers[self.oppcode_data.x as usize] == self.oppcode_data.nn
        {
            self.program_counter += 2;
        }
    }

    /// 0x4XNN: Skips the next instruction if register x does not equal NN.
    fn skip_if_x_not_equal(&mut self) -> ()
    {
        if self.registers[self.oppcode_data.x as usize] != self.oppcode_data.nn
        {
            self.program_counter += 2;
        }
    }

    /// 0x5XY0: Skips the next instruction if register x equals register y.
    fn skip_if_x_y_equal(&mut self) -> ()
    {
        if self.registers[self.oppcode_data.x as usize] == self.registers[self.oppcode_data.y as usize]
        {
            self.program_counter += 2;
        }
    }

    /// 0x6XNN: Assigns NN to register x.
    fn assign_nn(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] = self.oppcode_data.nn;
    }

    /// 0x7XNN: Adds NN to register x.
    fn add_nnn(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] += self.oppcode_data.nn;    
    }

    /// 0x8XY0: Assigns register y's value to register x.
    fn assign(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] = self.registers[self.oppcode_data.y as usize];
    }

    /// 0x8XY1: ORs register x and register y. Stores the result in register x.
    fn or(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] |= self.registers[self.oppcode_data.y as usize];
    }
    
    /// 0x8XY2: ANDs register x and register y. Stores the result in register x.
    fn and(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] &= self.registers[self.oppcode_data.y as usize];
    }
    
    ///  0x8XY3: XORs register x and register y. Stores the result in register x.
    fn xor(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] ^= self.registers[self.oppcode_data.y as usize];
    }
    
    /// 0x8XY4: Adds register y to register x. Stores the result in register x.    
    /// Sets register 0xF to 1 if an overflow occurs, sets it to 0 otherwise.
    fn add(&mut self) -> ()
    {
        let x_value = self.registers[self.oppcode_data.x as usize];
        let y_value = self.registers[self.oppcode_data.y as usize];
        let (result,overflow) = x_value.overflowing_add(y_value);

        self.registers[self.oppcode_data.x as usize] = result;
        self.registers[0xF] = if overflow {1} else {0};
    }

    /// 0x8XY5: Subtracts register y from register x. Stores the result in register x.
    /// Sets register 0xF to 1 if an overflow occurs, sets it to 0 otherwise.
    fn subtract_y_from_x(&mut self) -> ()
    {
        let x_value = self.registers[self.oppcode_data.x as usize];
        let y_value = self.registers[self.oppcode_data.y as usize];
        let (result,overflow) = x_value.overflowing_sub(y_value);
        
        self.registers[self.oppcode_data.x as usize] = result;
        self.registers[0xF] = if overflow {1} else {0}
    }
    
    /// 0x8XY6: Shifts register x one to the right. The eliminated bit is stored in register 0xF.
    fn shift_x_right(&mut self) -> ()
    {
        let least_significant_bit : u8 = self.registers[self.oppcode_data.x as usize] & 0x0001;
        
        self.registers[self.oppcode_data.x as usize] >>= 1;
        self.registers[0xF] = least_significant_bit;
    }
    
    /// 0x8XY7: Subtracts register x from register y. Stores the result in register x.
    /// Sets register 0xF to 1 if an overflow occurs, sets it to 0 otherwise.
    fn subtract_x_from_y(&mut self) -> ()
    {
        let x : u8 = self.registers[self.oppcode_data.x as usize];
        let y : u8 = self.registers[self.oppcode_data.y as usize];

        let (result, overflow) = y.overflowing_sub(x);

        self.registers[self.oppcode_data.x as usize] = result;
        self.registers[0xF] = if overflow {1} else {0};
    }
    
    /// 0x8XYE: Shifts register x one to the left. The eliminated bit is stored in register 0xF.
    fn shift_x_left(&mut self) -> ()
    {
        let most_significant_bit : u8 = self.registers[self.oppcode_data.x as usize] & 0x0080;
        
        self.registers[self.oppcode_data.x as usize] <<= 1;
        self.registers[0xF] = most_significant_bit;
    }

    /// 0x9XY0: Skips the next instruction if register x equals register y.
    fn skip_if_equals(&mut self) -> ()
    {
        if self.registers[self.oppcode_data.x as usize] == self.registers[self.oppcode_data.y as usize]
        {
            self.program_counter += 2;
        }
    }

    /// 0xANNN: Sets the index register (I) to NNN.
    fn set_index_register(&mut self) -> ()
    {
        self.index_register = self.oppcode_data.nnn;
    }

    /// 0xBNNN: Jumps the program counter to register 0 + NNN
    fn jump_to_address_plus_register_0(&mut self) -> ()
    {
        self.program_counter = self.oppcode_data.nnn + self.registers[0] as u16;
    }

    /// 0xCXNN: Generates a random number [0,255] and ANDs it with NN. Stores the result in register x.
    fn set_x_to_random_and(&mut self) -> ()
    {
        let nn :u8 = self.oppcode_data.nn as u8;
        let rn :u8 = rand::random::<u8>();

        self.registers[self.oppcode_data.x as usize] = nn & rn;
    }

    /// 0xDXYN: Draws the sprite at coordinates x and y.
    /// The sprite is 8 pixels wide and N pixels tall. 
    /// The sprite is read from main memory at the address that the index register (I) is pointing to.
    /// The drawn pixels are XORd with the screen content.
    /// If any pixels are flipped from set to unset then the register 0xF is set to 1. 
    /// Otherwise it is set to 0.
    fn draw_sprite(&mut self)  -> ()
    {
        self.registers[0xF] = 0;

        let x : u8 = self.registers[self.oppcode_data.x as usize];
        let y : u8 = self.registers[self.oppcode_data.y as usize];
        let n : u16 = self.current_opcode & 0x00FF;

        let sprite_memory = self.index_register;

        for y_line in 0..n
        {
            let pixel :u16 = self.memory[(sprite_memory +y_line) as usize] as u16;

            for x_line in 0..8
            {
                if (pixel &  (0x80 >> x_line)) != 0
                {
                    if self.texture[(x+x_line+((y+y_line as u8)*SCREEN_WIDTH)) as usize] == 1
                    {
                        self.registers[0xF] = 1;
                    }
                    self.texture[(x+x_line+((y+y_line as u8)*SCREEN_WIDTH)) as usize] ^= 1
                }
            }
        }

    }

    /// 0xEX9E: Skips the next instruction if the key stored in register x is pressed.
    fn skip_if_key_is_pressed(&mut self) -> ()
    {
        if self.keys[self.registers[self.oppcode_data.x as usize] as usize] != 0
        {
            self.program_counter += 2;
        }
    }

    /// 0xEXA1: Skips the next instruction if the key stored in register x is not pressed.
    fn skip_if_key_is_not_pressed(&mut self) -> ()
    {
        if self.keys[self.registers[self.oppcode_data.x as usize] as usize] == 0
        {
            self.program_counter += 2;
        }
    }

    /// 0xFX07: Stores the delay timer to register x.
    fn get_delay_timer(&mut self) -> ()
    {
        self.registers[self.oppcode_data.x as usize] = self.delay_timer;
    }

    /// 0xFX0A: Blocks execution untill a key press is received.
    /// Once a key press is received, the pressed key will be stored in register x.
    fn wait_for_key_press(&mut self) -> ()
    {
        if was_key_pressed()
        {
            self.registers[self.oppcode_data.x as usize] = get_first_pressed_key();
        }
        else
        {
            self.program_counter -= 2; // This means this command will be executed again next cycle.
        }
    }

    /// 0xFX15: Sets the delay timer to register x.
    fn set_delay_timer(&mut self) -> ()
    {
        self.delay_timer = self.registers[self.oppcode_data.x as usize];
    }

    /// 0xFX18: Sets the sound timer to register x.
    fn set_sound_timer(&mut self) -> ()
    {
        self.sound_timer = self.registers[self.oppcode_data.x as usize];
    }

    /// 0xFX1E: Adds register x to the index register.
    /// Sets register 0xF to 1 if an overflow occurs, sets it to 0 otherwise.
    fn add_to_index(&mut self) -> ()
    {
        let (result, overflow) = self.index_register.overflowing_add(self.oppcode_data.x as u16);
        self.index_register = result;
        self.registers[0xF] = if overflow {1} else {0} 
    }

    /// 0xFX29: Sets the index register (I) to the location of the sprite for the character in register x.
    /// Characters 0x0-0xF are represented by a 4x5 font.
    /// Each font sprite is 5 bytes in size.
    fn set_sprite_address(&mut self) -> ()
    {
        self.index_register = self.registers[self.oppcode_data.x as usize] as u16 * 5;
    }

    /// 0xFX33: Stores the decimal representation of register x and stores each character into
    /// memory at the address that the index register is pointing to (with a maximum of 3). 
    fn binary_coded_decimal(&mut self) -> ()
    {
        let base :usize = self.index_register as usize;
        self.memory[base + 0] = (self.registers[self.oppcode_data.x as usize] / 100) %10;
        self.memory[base + 1] = (self.registers[self.oppcode_data.x as usize] / 10) %10;
        self.memory[base + 2] = self.registers[self.oppcode_data.x as usize] %10;

    }

    /// 0xFX55: Stores the content of register 0-X (x inclusive) at main memory, starting at
    /// the addres at the index register (I).
    fn register_dump(&mut self) -> ()
    {
        let base: usize = self.index_register as usize; 
        for i in 0..=self.oppcode_data.x as usize
        {
            self.memory[base + i] = self.registers[i];
        }
    }
    
    /// 0xFX65: Loads the memory pointed at by the index register (I) into the registers 0-X(x inclusive).
    fn register_load(&mut self) -> ()
    {
        let base: usize = self.index_register as usize; 
        for i in 0..=self.oppcode_data.x as usize
        {
            self.registers[i] = self.memory[base + i];
        }
    }

}

fn main()
{
    let mut chip : Chip = Chip::new();
    // chip.load_font(&FONT_SET);
    println!("Hello");
    println!("Chip8");
}