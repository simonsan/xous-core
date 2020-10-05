use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::{BufRead, BufReader, Read, Write};

#[derive(Debug)]
pub enum ParseError {
    UnexpectedTag,
    MissingValue,
    ParseIntError,
    NonUTF8,
    WriteError,
}

#[derive(Default, Debug)]
pub struct Field {
    name: String,
    lsb: usize,
    msb: usize,
}

#[derive(Default, Debug)]
pub struct Register {
    name: String,
    offset: usize,
    description: Option<String>,
    fields: Vec<Field>,
}

#[derive(Default, Debug)]
pub struct Interrupt {
    name: String,
    value: usize,
}

#[derive(Default, Debug)]
pub struct Peripheral {
    name: String,
    pub base: usize,
    size: usize,
    interrupt: Vec<Interrupt>,
    registers: Vec<Register>,
}

#[derive(Default, Debug)]
pub struct MemoryRegion {
    pub name: String,
    pub base: usize,
    pub size: usize,
}

#[derive(Default, Debug)]
pub struct Description {
    pub peripherals: Vec<Peripheral>,
    pub memory_regions: Vec<MemoryRegion>,
}

pub fn get_base(value: &str) -> (&str, u32) {
    if value.starts_with("0x") {
        (value.trim_start_matches("0x"), 16)
    } else if value.starts_with("0X") {
        (value.trim_start_matches("0X"), 16)
    } else if value.starts_with("0b") {
        (value.trim_start_matches("0b"), 2)
    } else if value.starts_with("0B") {
        (value.trim_start_matches("0B"), 2)
    } else if value.starts_with('0') && value != "0" {
        (value.trim_start_matches('0'), 8)
    } else {
        (value, 10)
    }
}

fn parse_usize(value: &[u8]) -> Result<usize, ParseError> {
    let value_as_str = String::from_utf8(value.to_vec()).or(Err(ParseError::NonUTF8))?;
    let (value, base) = get_base(&value_as_str);
    usize::from_str_radix(value, base).or(Err(ParseError::ParseIntError))
}

fn extract_contents<T: BufRead>(reader: &mut Reader<T>) -> Result<String, ParseError> {
    let mut buf = Vec::new();
    let contents = reader
        .read_event(&mut buf)
        .map_err(|_| ParseError::UnexpectedTag)?;
    match contents {
        Event::Text(t) => t
            .unescape_and_decode(reader)
            .map_err(|_| ParseError::NonUTF8),
        _ => Err(ParseError::UnexpectedTag),
    }
}

fn generate_field<T: BufRead>(reader: &mut Reader<T>) -> Result<Field, ParseError> {
    let mut buf = Vec::new();
    let mut name = None;
    let mut lsb = None;
    let mut msb = None;
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e
                    .unescape_and_decode(reader)
                    .map_err(|_| ParseError::NonUTF8)?;
                match tag_name.as_str() {
                    "name" => name = Some(extract_contents(reader)?),
                    "lsb" => lsb = Some(parse_usize(extract_contents(reader)?.as_bytes())?),
                    "msb" => msb = Some(parse_usize(extract_contents(reader)?.as_bytes())?),
                    _ => (),
                }
            }
            Ok(Event::End(ref e)) => {
                if let b"field" = e.name() {
                    break;
                }
            }
            Ok(_) => (),
            Err(e) => panic!("error parsing: {:?}", e),
        }
    }

    Ok(Field {
        name: name.ok_or(ParseError::MissingValue)?,
        lsb: lsb.ok_or(ParseError::MissingValue)?,
        msb: msb.ok_or(ParseError::MissingValue)?,
    })
}

fn generate_fields<T: BufRead>(
    reader: &mut Reader<T>,
    fields: &mut Vec<Field>,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"field" => fields.push(generate_field(reader)?),
                _ => panic!("unexpected tag in <field>: {:?}", e),
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"fields" => {
                    // println!("End fields");
                    break;
                }
                e => panic!("unhandled value: {:?}", e),
            },
            Ok(Event::Text(_)) => (),
            e => panic!("unhandled value: {:?}", e),
        }
    }
    Ok(())
}

fn generate_register<T: BufRead>(reader: &mut Reader<T>) -> Result<Register, ParseError> {
    let mut buf = Vec::new();
    let mut name = None;
    let mut offset = None;
    let description = None;
    let mut fields = vec![];
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e
                    .unescape_and_decode(reader)
                    .map_err(|_| ParseError::NonUTF8)?;
                match tag_name.as_str() {
                    "name" => name = Some(extract_contents(reader)?),
                    "addressOffset" => {
                        offset = Some(parse_usize(extract_contents(reader)?.as_bytes())?)
                    }
                    "fields" => generate_fields(reader, &mut fields)?,
                    _ => (),
                }
            }
            Ok(Event::End(ref e)) => {
                if let b"register" = e.name() {
                    break;
                }
            }
            Ok(_) => (),
            Err(e) => panic!("error parsing: {:?}", e),
        }
    }

    Ok(Register {
        name: name.ok_or(ParseError::MissingValue)?,
        offset: offset.ok_or(ParseError::MissingValue)?,
        description,
        fields,
    })
}

fn generate_interrupts<T: BufRead>(
    reader: &mut Reader<T>,
    interrupts: &mut Vec<Interrupt>,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    let mut name = None;
    let mut value = None;
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e
                    .unescape_and_decode(reader)
                    .map_err(|_| ParseError::NonUTF8)?;
                match tag_name.as_str() {
                    "name" => name = Some(extract_contents(reader)?),
                    "value" => {
                        value = Some(parse_usize(extract_contents(reader)?.as_bytes())?)
                    }
                    _ => (),
                }
            }
            Ok(Event::End(ref e)) => {
                if let b"interrupt" = e.name() {
                    break;
                }
            }
            Ok(_) => (),
            Err(e) => panic!("error parsing: {:?}", e),
        }
    }

    interrupts.push(
        Interrupt {
            name: name.ok_or(ParseError::MissingValue)?,
            value: value.ok_or(ParseError::MissingValue)?,
        });

    Ok(())
}

fn generate_registers<T: BufRead>(
    reader: &mut Reader<T>,
    registers: &mut Vec<Register>,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"register" => registers.push(generate_register(reader)?),
                _ => panic!("unexpected tag in <registers>: {:?}", e),
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"registers" => {
                    break;
                }
                e => panic!("unhandled value: {:?}", e),
            },
            Ok(Event::Text(_)) => (),
            e => panic!("unhandled value: {:?}", e),
        }
    }
    Ok(())
}

fn generate_peripheral<T: BufRead>(reader: &mut Reader<T>) -> Result<Peripheral, ParseError> {
    let mut buf = Vec::new();
    let mut name = None;
    let mut base = None;
    let mut size = None;
    let mut registers = vec![];
    let mut interrupts = vec![];
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e
                    .unescape_and_decode(reader)
                    .map_err(|_| ParseError::NonUTF8)?;
                match tag_name.as_str() {
                    "name" => name = Some(extract_contents(reader)?),
                    "baseAddress" => {
                        base = Some(parse_usize(extract_contents(reader)?.as_bytes())?)
                    }
                    "size" => size = Some(parse_usize(extract_contents(reader)?.as_bytes())?),
                    "registers" => generate_registers(reader, &mut registers)?,
                    "interrupt" => generate_interrupts(reader, &mut interrupts)?,
                    _ => (),
                }
            }
            Ok(Event::End(ref e)) => {
                if let b"peripheral" = e.name() {
                    break;
                }
            }
            Ok(_) => (),
            Err(e) => panic!("error parsing: {:?}", e),
        }
    }

    Ok(Peripheral {
        name: name.ok_or(ParseError::MissingValue)?,
        base: base.ok_or(ParseError::MissingValue)?,
        size: size.ok_or(ParseError::MissingValue)?,
        interrupt: interrupts,
        registers,
    })
}

fn generate_peripherals<T: BufRead>(reader: &mut Reader<T>) -> Result<Vec<Peripheral>, ParseError> {
    let mut buf = Vec::new();
    let mut peripherals = vec![];
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"peripheral" => peripherals.push(generate_peripheral(reader)?),
                _ => panic!("unexpected tag in <peripherals>: {:?}", e),
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"peripherals" => {
                    break;
                }
                e => panic!("unhandled value: {:?}", e),
            },
            Ok(Event::Text(_)) => (),
            e => panic!("unhandled value: {:?}", e),
        }
    }
    Ok(peripherals)
}

fn generate_memory_region<T: BufRead>(reader: &mut Reader<T>) -> Result<MemoryRegion, ParseError> {
    let mut buf = Vec::new();
    let mut name = None;
    let mut base = None;
    let mut size = None;

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e
                    .unescape_and_decode(reader)
                    .map_err(|_| ParseError::NonUTF8)?;
                match tag_name.as_str() {
                    "name" => name = Some(extract_contents(reader)?),
                    "baseAddress" => {
                        base = Some(parse_usize(extract_contents(reader)?.as_bytes())?)
                    }
                    "size" => size = Some(parse_usize(extract_contents(reader)?.as_bytes())?),
                    _ => (),
                }
            }
            Ok(Event::End(ref e)) => {
                if let b"memoryRegion" = e.name() {
                    break;
                }
            }
            Ok(_) => (),
            Err(e) => panic!("error parsing: {:?}", e),
        }
    }

    Ok(MemoryRegion {
        name: name.ok_or(ParseError::MissingValue)?,
        base: base.ok_or(ParseError::MissingValue)?,
        size: size.ok_or(ParseError::MissingValue)?,
    })
}

fn parse_memory_regions<T: BufRead>(
    reader: &mut Reader<T>,
    description: &mut Description,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"memoryRegion" => description
                    .memory_regions
                    .push(generate_memory_region(reader)?),
                _ => panic!("unexpected tag in <memoryRegions>: {:?}", e),
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"memoryRegions" => {
                    break;
                }
                e => panic!("unhandled value: {:?}", e),
            },
            Ok(Event::Text(_)) => (),
            e => panic!("unhandled value: {:?}", e),
        }
    }
    Ok(())
}

fn parse_vendor_extensions<T: BufRead>(
    reader: &mut Reader<T>,
    description: &mut Description,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"memoryRegions" => parse_memory_regions(reader, description)?,
                _ => panic!("unexpected tag in <vendorExtensions>: {:?}", e),
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"vendorExtensions" => {
                    break;
                }
                e => panic!("unhandled value: {:?}", e),
            },
            Ok(Event::Text(_)) => (),
            e => panic!("unhandled value: {:?}", e),
        }
    }
    Ok(())
}

fn print_header<U: Write>(out: &mut U) -> std::io::Result<()> {
    let s = r####"
use core::convert::TryInto;
pub struct Register {
    /// Offset of this register within this CSR
    offset: usize,
}
impl Register {
    pub const fn new(offset: usize) -> Register {
        Register { offset }
    }
}
pub struct Field {
    /// A bitmask we use to AND to the value, unshifted.
    /// E.g. for a width of `3` bits, this mask would be 0b111.
    mask: usize,
    /// Offset of the first bit in this field
    offset: usize,
    /// A copy of the register address that this field
    /// is a member of. Ideally this is optimized out by the
    /// compiler.
    register: Register,
}
impl Field {
    /// Define a new CSR field with the given width at a specified
    /// offset from the start of the register.
    pub const fn new(width: usize, offset: usize, register: Register) -> Field {
        // Asserts don't work in const fn yet.
        // assert!(width != 0, "field width cannot be 0");
        // assert!((width + offset) < 32, "field with and offset must fit within a 32-bit value");
        // It would be lovely if we could call `usize::pow()` in a const fn.
        let mask = match width {
            0 => 0,
            1 => 1,
            2 => 3,
            3 => 7,
            4 => 15,
            5 => 31,
            6 => 63,
            7 => 127,
            8 => 255,
            9 => 511,
            10 => 1023,
            11 => 2047,
            12 => 4095,
            13 => 8191,
            14 => 16383,
            15 => 32767,
            16 => 65535,
            17 => 131071,
            18 => 262143,
            19 => 524287,
            20 => 1048575,
            21 => 2097151,
            22 => 4194303,
            23 => 8388607,
            24 => 16777215,
            25 => 33554431,
            26 => 67108863,
            27 => 134217727,
            28 => 268435455,
            29 => 536870911,
            30 => 1073741823,
            31 => 2147483647,
            _ => 0,
        };
        Field {
            mask,
            offset,
            register,
        }
    }
}
pub struct CSR<T> {
    base: *mut T,
}
impl<T> CSR<T>
where
    T: core::convert::TryFrom<usize> + core::convert::TryInto<usize> + core::default::Default,
{
    pub fn new(base: *mut T) -> Self {
        CSR { base }
    }
    /// Read the contents of this register
    pub fn r(&mut self, reg: Register) -> T {
        let usize_base: *mut usize = unsafe { core::mem::transmute(self.base) };
        unsafe { usize_base.add(reg.offset).read_volatile() }
            .try_into()
            .unwrap_or_default()
    }
    /// Read a field from this CSR
    pub fn rf(&mut self, field: Field) -> T {
        let usize_base: *mut usize = unsafe { core::mem::transmute(self.base) };
        ((unsafe { usize_base.add(field.register.offset).read_volatile() } >> field.offset)
            & field.mask)
            .try_into()
            .unwrap_or_default()
    }
    /// Read-modify-write a given field in this CSR
    pub fn rmwf(&mut self, field: Field, value: T) {
        let usize_base: *mut usize = unsafe { core::mem::transmute(self.base) };
        let value_as_usize: usize = value.try_into().unwrap_or_default() << field.offset;
        let previous =
            unsafe { usize_base.add(field.register.offset).read_volatile() } & !field.mask;
        unsafe {
            usize_base
                .add(field.register.offset)
                .write_volatile(previous | value_as_usize)
        };
    }
    /// Write a given field without reading it first
    pub fn wfo(&mut self, field: Field, value: T) {
        let usize_base: *mut usize = unsafe { core::mem::transmute(self.base) };
        let value_as_usize: usize = (value.try_into().unwrap_or_default() & field.mask) << field.offset;
        unsafe {
            usize_base
                .add(field.register.offset)
                .write_volatile(value_as_usize)
        };
    }
    /// Write the entire contents of a register without reading it first
    pub fn wo(&mut self, reg: Register, value: T) {
        let usize_base: *mut usize = unsafe { core::mem::transmute(self.base) };
        let value_as_usize: usize = value.try_into().unwrap_or_default();
        unsafe { usize_base.add(reg.offset).write_volatile(value_as_usize) };
    }
    /// Zero a field from a provided value
    pub fn zf(&mut self, field: Field, value: T) -> T {
        let value_as_usize: usize = value.try_into().unwrap_or_default();
        (value_as_usize & !(field.mask << field.offset))
            .try_into()
            .unwrap_or_default()
    }
    /// Shift & mask a value to its final field position
    pub fn ms(&mut self, field: Field, value: T) -> T {
        let value_as_usize: usize = value.try_into().unwrap_or_default();
        ((value_as_usize & field.mask) << field.offset)
            .try_into()
            .unwrap_or_default()
    }
}
"####;
    out.write_all(s.as_bytes())
}

fn print_memory_regions<U: Write>(regions: &[MemoryRegion], out: &mut U) -> std::io::Result<()> {
    writeln!(out, "// Physical base addresses of memory regions")?;
    for region in regions {
        writeln!(
            out,
            "pub const HW_{}_MEM:     usize = 0x{:08x};",
            region.name, region.base
        )?;
        writeln!(
            out,
            "pub const HW_{}_MEM_LEN: usize = {};",
            region.name, region.size
        )?;
    }
    writeln!(out)?;
    Ok(())
}

fn print_peripherals<U: Write>(peripherals: &[Peripheral], out: &mut U) -> std::io::Result<()> {
    writeln!(out, "// Physical base addresses of registers")?;
    for peripheral in peripherals {
        writeln!(
            out,
            "pub const HW_{}_BASE :   usize = {};",
            peripheral.name.to_uppercase(), peripheral.base
        )?;
    }
    writeln!(out)?;

    writeln!(out, "pub mod utra {{")?;
    for peripheral in peripherals {
        writeln!(out)?;
        writeln!(out, "    pub mod {} {{", peripheral.name.to_lowercase())?;
        for register in &peripheral.registers {
            writeln!(out)?;
            if let Some(description) = &register.description {
                writeln!(out, "        /// {}", description)?;
            }
            writeln!(
                out,
                "        pub const {}: crate::Register = crate::Register::new({});",
                register.name.to_uppercase(), register.offset
            )?;
            for field in &register.fields {
                writeln!(
                    out,
                    "        pub const {}_{}: crate::Field = crate::Field::new({}, {}, {});",
                    register.name,
                    field.name.to_uppercase(),
                    field.msb + 1 - field.lsb,
                    field.lsb,
                    register.name
                )?;
            }
        }
        writeln!(out)?;
        for interrupt in &peripheral.interrupt {
            writeln!(
                out,
                "        pub const {}_IRQ: usize = {};",
                interrupt.name.to_uppercase(),
                interrupt.value
            )?;
        }
        writeln!(out, "    }}")?;
    }
    writeln!(out, "}}")?;
    Ok(())
}

fn print_tests<U: Write>(peripherals: &[Peripheral], out: &mut U) -> std::io::Result<()> {
    let test_header = r####"
#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn compile_check() {
        use super::*;
"####.as_bytes();
    out.write_all(test_header)?;
    for peripheral in peripherals {
        let mod_name = peripheral.name.to_lowercase();
        let per_name = peripheral.name.to_lowercase() + "_csr";
        writeln!(out, "        let mut {} = CSR::new(HW_{}_BASE as *mut u32);", per_name, peripheral.name.to_uppercase())?;
        for register in &peripheral.registers {
            writeln!(out)?;
            let reg_name = register.name.to_uppercase();
            writeln!(out, "        let foo = {}.r(utra::{}::{});", per_name, mod_name, reg_name)?;
            writeln!(out, "        {}.wo(utra::{}::{}, foo);", per_name, mod_name, reg_name)?;
            for field in &register.fields {
                let field_name = format!("{}_{}", reg_name, field.name.to_uppercase());
                writeln!(out, "        let bar = {}.rf(utra::{}::{});", per_name, mod_name, field_name)?;
                writeln!(out, "        {}.rmwf(utra::{}::{}, bar);", per_name, mod_name, field_name)?;
                writeln!(out, "        let mut baz = {}.zf(utra::{}::{}, bar);", per_name, mod_name, field_name)?;
                writeln!(out, "        baz |= {}.ms(utra::{}::{}, 1);", per_name, mod_name, field_name)?;
                writeln!(out, "        {}.wfo(utra::{}::{}, baz);", per_name, mod_name, field_name)?;
            }
        }
    }
    writeln!(out, "    }}")?;
    writeln!(out, "}}")?;
    Ok(())
}

pub fn parse_svd<T: Read>(src: T) -> Result<Description, ParseError> {
    let mut buf = Vec::new();
    let buf_reader = BufReader::new(src);
    let mut reader = Reader::from_reader(buf_reader);
    let mut description = Description::default();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"peripherals" => {
                    description.peripherals = generate_peripherals(&mut reader)?;
                }
                b"vendorExtensions" => {
                    parse_vendor_extensions(&mut reader, &mut description)?;
                }
                _ => (),
            },
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }
    Ok(description)
}

pub fn generate<T: Read, U: Write>(src: T, dest: &mut U) -> Result<(), ParseError> {
    let description = parse_svd(src)?;

    print_header(dest).or(Err(ParseError::WriteError))?;
    print_memory_regions(&description.memory_regions, dest).or(Err(ParseError::WriteError))?;
    print_peripherals(&description.peripherals, dest).or(Err(ParseError::WriteError))?;
    print_tests(&description.peripherals, dest).or(Err(ParseError::WriteError))?;

    Ok(())
}