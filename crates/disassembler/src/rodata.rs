use {
    serde::{Deserialize, Serialize},
    std::collections::BTreeSet,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RodataType {
    Ascii(String),
    Byte(Vec<i8>),
    Word(i16),
    Long(i32),
    Quad(i64),
}

impl RodataType {
    pub fn to_asm(&self) -> String {
        match self {
            RodataType::Ascii(s) => format!(".ascii \"{}\"", s),
            RodataType::Byte(v) => format!(".byte {}", format_byte_values(v)),
            RodataType::Word(v) => format!(".word 0x{:04x}", *v as u16),
            RodataType::Long(v) => format!(".long 0x{:08x}", *v as u32),
            RodataType::Quad(v) => format!(".quad 0x{:016x}", *v as u64),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RodataItem {
    pub label: String,
    pub offset: u64,
    pub size: u64,
    pub data_type: RodataType,
    pub data: Vec<u8>,
}

impl RodataItem {
    pub fn new(label: String, offset: u64, data: Vec<u8>, data_type: RodataType) -> Self {
        Self {
            label,
            size: data.len() as u64,
            offset,
            data_type,
            data,
        }
    }

    pub fn to_asm(&self) -> String {
        format!("{}: {}", self.label, self.data_type.to_asm())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RodataSection {
    pub base_address: u64,      // base virtual address of the rodata section
    pub data: Vec<u8>,          // raw section data bytes
    pub items: Vec<RodataItem>, // parsed rodata items
}

impl RodataSection {
    pub fn parse(data: Vec<u8>, base_address: u64, references: &BTreeSet<u64>) -> Self {
        let items = parse_rodata_items(&data, base_address, references);
        Self {
            base_address,
            data,
            items,
        }
    }

    #[inline]
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn to_asm(&self) -> String {
        if self.items.is_empty() {
            return String::new();
        }

        let mut output = String::from(".rodata\n");
        for item in &self.items {
            output.push_str(&format!("  {}\n", item.to_asm()));
        }
        output
    }

    pub fn get_label(&self, address: u64) -> Option<&str> {
        if address < self.base_address {
            return None;
        }
        let offset = address - self.base_address;
        self.items
            .iter()
            .find(|item| item.offset == offset)
            .map(|item| item.label.as_str())
    }

    #[inline]
    pub fn contains_address(&self, address: u64) -> bool {
        address >= self.base_address && address < self.base_address + self.data.len() as u64
    }
}

fn parse_rodata_items(
    data: &[u8],
    base_address: u64,
    references: &BTreeSet<u64>,
) -> Vec<RodataItem> {
    if data.is_empty() {
        return Vec::new();
    }

    // Convert absolute addresses to relative offsets within rodata.
    let mut offsets: Vec<u64> = references
        .iter()
        .filter_map(|&addr| {
            if addr >= base_address && addr < base_address + data.len() as u64 {
                Some(addr - base_address)
            } else {
                None
            }
        })
        .collect();

    // Treat entire rodata as one item if there are no references.
    if offsets.is_empty() {
        let trimmed = trim_trailing_zeros(data);
        if trimmed.is_empty() {
            return Vec::new();
        }
        let data_type = infer_type(trimmed);
        let label = generate_label(0, &data_type);
        return vec![RodataItem::new(label, 0, trimmed.to_vec(), data_type)];
    }

    // Add offset 0 if the first reference isn't at the start.
    if offsets[0] != 0 {
        offsets.insert(0, 0);
    }

    // Create items from segments between consecutive offsets.
    let mut items = Vec::new();
    for (i, &offset) in offsets.iter().enumerate() {
        let start = offset as usize;
        if start >= data.len() {
            continue;
        }

        // End is either the next offset or the end of data
        let end = if i + 1 < offsets.len() {
            (offsets[i + 1] as usize).min(data.len())
        } else {
            let remaining = &data[start..];
            start + trim_trailing_zeros(remaining).len()
        };

        if start < end {
            let bytes = data[start..end].to_vec();
            let data_type = infer_type(&bytes);
            let label = generate_label(offset, &data_type);
            items.push(RodataItem::new(label, offset, bytes, data_type));
        }
    }

    items
}

#[inline]
fn trim_trailing_zeros(data: &[u8]) -> &[u8] {
    let end = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    &data[..end]
}

fn infer_type(data: &[u8]) -> RodataType {
    if let Ok(s) = std::str::from_utf8(data)
        && is_ascii(s)
        && !s.is_empty()
    {
        return RodataType::Ascii(s.to_string());
    }

    match data.len() {
        2 => RodataType::Word(i16::from_le_bytes([data[0], data[1]])),
        4 => RodataType::Long(i32::from_le_bytes(data[0..4].try_into().unwrap())),
        8 => RodataType::Quad(i64::from_le_bytes(data[0..8].try_into().unwrap())),
        _ => RodataType::Byte(data.iter().map(|&b| b as i8).collect()),
    }
}

#[inline]
fn is_ascii(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_graphic() || c == ' ' || c == '\t' || c == '\n' || c == '\r')
}

fn generate_label(offset: u64, data_type: &RodataType) -> String {
    match data_type {
        RodataType::Ascii(_) => format!("str_{:04x}", offset),
        _ => format!("data_{:04x}", offset),
    }
}

fn format_byte_values(vals: &[i8]) -> String {
    vals.iter()
        .map(|&v| format!("0x{:02x}", v as u8))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_type_ascii() {
        let data = b"Hello, World!";
        let result = infer_type(data);
        assert!(matches!(result, RodataType::Ascii(s) if s == "Hello, World!"));
    }

    #[test]
    fn test_infer_type_byte() {
        let data = &[0x01];
        if let RodataType::Byte(vals) = infer_type(data) {
            assert_eq!(vals[0], 0x01);
        } else {
            panic!("Expected Byte type");
        }
    }

    #[test]
    fn test_infer_type_word() {
        let data = &[0x34, 0x12];
        if let RodataType::Word(val) = infer_type(data) {
            assert_eq!(val, 0x1234);
        } else {
            panic!("Expected Word type");
        }
    }

    #[test]
    fn test_infer_type_long() {
        let data = &[0x78, 0x56, 0x34, 0x12];
        if let RodataType::Long(val) = infer_type(data) {
            assert_eq!(val, 0x12345678);
        } else {
            panic!("Expected Long type");
        }
    }

    #[test]
    fn test_infer_type_quad() {
        let data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        if let RodataType::Quad(val) = infer_type(data) {
            assert_eq!(val, 0x0807060504030201i64);
        } else {
            panic!("Expected Quad type");
        }
    }

    #[test]
    fn test_infer_type_bytes() {
        let data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x0];
        if let RodataType::Byte(vals) = infer_type(data) {
            assert_eq!(vals.len(), 9);
        } else {
            panic!("Expected Byte array for 9 bytes");
        }
    }

    #[test]
    fn test_generate_label_str() {
        let t = RodataType::Ascii("test".to_string());
        assert_eq!(generate_label(0, &t), "str_0000");
        assert_eq!(generate_label(16, &t), "str_0010");
        assert_eq!(generate_label(255, &t), "str_00ff");
    }

    #[test]
    fn test_generate_label_data() {
        assert_eq!(generate_label(0, &RodataType::Byte(vec![0])), "data_0000");
        assert_eq!(generate_label(0, &RodataType::Word(0)), "data_0000");
        assert_eq!(generate_label(0, &RodataType::Long(0)), "data_0000");
        assert_eq!(generate_label(0, &RodataType::Quad(0)), "data_0000");
    }

    #[test]
    fn test_rodata_type_to_asm() {
        assert_eq!(
            RodataType::Ascii("Hello".to_string()).to_asm(),
            ".ascii \"Hello\""
        );
        assert_eq!(
            RodataType::Byte(vec![0, 1, -1]).to_asm(),
            ".byte 0x00, 0x01, 0xff"
        );
        assert_eq!(RodataType::Word(0x1234).to_asm(), ".word 0x1234");
        assert_eq!(RodataType::Long(0x12345678).to_asm(), ".long 0x12345678");
        assert_eq!(
            RodataType::Quad(0x123456789ABCDEF0u64 as i64).to_asm(),
            ".quad 0x123456789abcdef0"
        );
    }

    #[test]
    fn test_rodata_item_to_asm() {
        let item = RodataItem::new(
            "str_0000".to_string(),
            0,
            b"Hello".to_vec(),
            RodataType::Ascii("Hello".to_string()),
        );
        assert_eq!(item.to_asm(), "str_0000: .ascii \"Hello\"");
    }

    #[test]
    fn test_rodata_section_empty() {
        let section = RodataSection::parse(Vec::new(), 0x100, &BTreeSet::new());
        assert!(section.to_asm().is_empty());
    }

    #[test]
    fn test_rodata_section_contains_address() {
        let section = RodataSection::parse(vec![0x01, 0x02, 0x03, 0x04], 0x100, &BTreeSet::new());

        assert!(section.contains_address(0x100));
        assert!(section.contains_address(0x103));
        assert!(!section.contains_address(0x99));
        assert!(!section.contains_address(0x104));
    }

    #[test]
    fn test_rodata_section_has_items() {
        let section_with_data = RodataSection::parse(vec![0x01], 0x100, &BTreeSet::new());
        assert!(section_with_data.has_items());

        let section_empty = RodataSection::parse(Vec::new(), 0x100, &BTreeSet::new());
        assert!(!section_empty.has_items());
    }

    #[test]
    fn test_trim_trailing_zeros() {
        assert_eq!(trim_trailing_zeros(&[1, 2, 3, 0, 0]), &[1, 2, 3]);
        assert_eq!(trim_trailing_zeros(&[0, 0, 0]), &[] as &[u8]);
        assert_eq!(trim_trailing_zeros(&[1, 0, 2, 0]), &[1, 0, 2]);
        assert_eq!(trim_trailing_zeros(&[]), &[] as &[u8]);
    }
}
