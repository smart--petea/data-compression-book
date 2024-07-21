use std::fs::File;
use std::io::{Result, Error, ErrorKind, Write, Read};
use std::path::Path;

pub struct BitFile {
    file: File,
    mask: u8, //contains a single bit mask used either to set or clear the current output bit or to mask in the current input bit
    rack: Vec<u8>, //contains the current byte of data either read in from the file or waiting to be written out to the file
    pacifier_counter: i32
}

impl BitFile {
    /**
     * The input and output routines in BITIO.H also have a paicifier feature that can be useful in
     * testing compression code. Every BIL_FILe structure has a pacifier_counter that gets
     * incremented every time a new byte is read in or written out to the corresponding file.
     * Once every 2048 bytes, a single character is written to stdout. This helps assure the 
     * impatient user that real work is being done.
     */
    pub fn open<P: AsRef<Path>>(path: P) -> Result<BitFile> {
        Ok(BitFile {
            file: File::open(path)?,
            mask: 0x80,
            rack: vec![0],
            pacifier_counter: 0,
        })
    }

    pub fn create<P: AsRef<Path>>(path: P) -> Result<BitFile> {
        Ok(BitFile {
            file: File::create(path)?,
            mask: 0x80,
            rack: vec![0],
            pacifier_counter: 0,
        })
    }

    pub fn flush_bits(&mut self) -> Result<()> {
        if self.mask != 0x80 {
            println!("write {:x} {:#8b}", self.rack[0], self.rack[0]);
            self.file.write(&self.rack)?; 
            println!("flush");
            self.file.flush()?;
        }

        Ok(())
    }

    pub fn output_bit(&mut self, bit: u8) -> Result<()> {
        if bit > 0 {
            self.rack[0] |= self.mask;
        }

        self.mask >>= 1;
        if self.mask == 0 {
            self.file.write(&self.rack)?;
            self.rack[0] = 0;
            self.mask = 0x80;
            self.pacifier_counter += 1;
            if self.pacifier_counter & 2047 == 0 {
                println!(".");
            }
        }

        Ok(())
    }

    pub fn output_bits(&mut self, code: u32, count: usize) -> Result<()> {
        let mut mask = 1u32 << (count - 1);

        while mask != 0 {
            if mask & code != 0 {
                self.rack[0] |= self.mask;
            }

            self.mask >>= 1;
            if self.mask == 0 {
                self.file.write(&self.rack)?;
                self.rack[0] = 0;
                self.mask = 0x80;
                self.pacifier_counter += 1;
                if self.pacifier_counter & 2047 == 0 {
                    println!(".");
                }
            }

            mask >>= 1;
        }

        Ok(())
    }

    pub fn input_bit(&mut self) -> Result<u8> {
        if self.mask == 0x80 {
            if self.file.read(&mut self.rack)? == 0 {
                return Err(Error::new(ErrorKind::Other, "EOF reached in InputBit!"));
            }

            self.pacifier_counter += 1;
            if self.pacifier_counter & 2047 == 0 {
                println!(".");
            }
        }

        let value = self.rack[0] & self.mask;
        self.mask >>= 1;
        if self.mask == 0 {
            self.mask = 0x80;
        }

        if value != 0 {
            return Ok(1);
        }

        Ok(0)
    }

    pub fn input_bits(&mut self, bit_count: u8) -> Result<u32> {
        let mut mask = 1u32 << (bit_count - 1);
        let mut return_value = 0u32;
        while mask != 0 {
            if self.mask == 0x80 {
                if self.file.read(&mut self.rack)? == 0 {
                    return Err(Error::new(ErrorKind::Other, "EOF reached in input_bits"));
                }

                self.pacifier_counter += 1;
                if self.pacifier_counter & 2047 == 0 {
                    println!(".");
                }
            }


            if self.rack[0] & self.mask != 0 {
                return_value |= mask;
            }

            self.mask >>= 1;
            if self.mask == 0 {
                self.mask = 0x80;
            }

            mask >>= 1;
        }

        Ok(return_value)
    }
}

pub fn file_print_binary(file: &mut dyn Write, code: u32, bits: usize) -> Result<()> {
    let mut mask = 1u32 << (bits - 1);
    let mut buffer = vec![0u8];
    while mask != 0 {
        if code & mask != 0 {
            buffer[0] = b'1';
        } else {
            buffer[0] = b'0';
        }

        if file.write(&buffer)? == 0 {
            return Err(Error::new(ErrorKind::Other, "the file does not accept new bytes"));
        }

        mask >>= 1;
    }

    Ok(())
}

impl Write for BitFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.file.flush()
    }
}
