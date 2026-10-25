//! Генератор безобидных синтетических jar/class для тестов сканера.
//!
//! Классы минимальны: валидный заголовок + constant pool с нужными константами
//! (строки, ссылки на методы и поля), кода нет — правилам сканера достаточно
//! пула констант, а cafebabe обязан такой файл разбирать как настоящий.
//! Реальная малварь не используется и не скачивается.

use std::collections::HashMap;
use std::io::Write;

/// major version синтетических классов: Java 8.
pub(crate) const MAJOR: u16 = 52;

/// Теги constant pool, которые умеет собирать генератор.
const TAG_UTF8: u8 = 1;
const TAG_INT: u8 = 3;
const TAG_STRING: u8 = 8;
const TAG_CLASS: u8 = 7;
const TAG_FIELDREF: u8 = 9;
const TAG_METHODREF: u8 = 10;
const TAG_IFACEMETHODREF: u8 = 11;
const TAG_NAMEANDTYPE: u8 = 12;

/// Собранный элемент пула: тег + готовые байты записи.
struct PoolEntry {
    tag: u8,
    payload: Vec<u8>,
}

#[allow(clippy::type_complexity)]
pub(crate) struct ClassBuilder {
    name: String,
    super_class: String,
    interfaces: Vec<String>,
    /// (флаги доступа, имя, дескриптор) объявленных методов.
    methods: Vec<(u16, String, String)>,
    pool: Vec<PoolEntry>,
    /// Дедупликация: (тег, ключ) -> индекс в пуле (1-based).
    lookup: HashMap<(u8, String), u16>,
}

fn u2(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

fn put(out: &mut Vec<u8>, tag: u8, mut payload: Vec<u8>) {
    out.push(tag);
    out.append(&mut payload);
}

impl ClassBuilder {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            super_class: "java/lang/Object".into(),
            interfaces: Vec::new(),
            methods: Vec::new(),
            pool: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    fn add(&mut self, tag: u8, key: String, payload: Vec<u8>) -> u16 {
        let entry = (tag, key.clone());
        if let Some(&i) = self.lookup.get(&entry) {
            return i;
        }
        self.pool.push(PoolEntry { tag, payload });
        let idx = self.pool.len() as u16;
        self.lookup.insert(entry, idx);
        idx
    }

    /// CONSTANT_Utf8.
    fn utf8(&mut self, s: &str) -> u16 {
        let mut payload = Vec::with_capacity(2 + s.len());
        payload.extend_from_slice(&u2(s.len() as u16));
        payload.extend_from_slice(s.as_bytes());
        self.add(TAG_UTF8, s.to_string(), payload)
    }

    /// CONSTANT_Integer.
    pub(crate) fn int_constant(mut self, v: i32) -> Self {
        self.add(TAG_INT, format!("int:{v}"), v.to_be_bytes().to_vec());
        self
    }

    /// CONSTANT_String со значением `s`.
    pub(crate) fn string(mut self, s: &str) -> Self {
        let s_idx = self.utf8(s);
        self.add(
            TAG_STRING,
            s.to_string(),
            u2(s_idx).to_vec(),
        );
        self
    }

    /// CONSTANT_Class.
    fn class(&mut self, name: &str) -> u16 {
        let n_idx = self.utf8(name);
        self.add(TAG_CLASS, name.to_string(), u2(n_idx).to_vec())
    }

    /// CONSTANT_NameAndType.
    fn nat(&mut self, name: &str, desc: &str) -> u16 {
        let n_idx = self.utf8(name);
        let d_idx = self.utf8(desc);
        let key = format!("{name}\0{desc}");
        let payload = [u2(n_idx), u2(d_idx)].concat();
        self.add(TAG_NAMEANDTYPE, key, payload)
    }

    /// Ссылка на метод (Methodref или InterfaceMethodref).
    fn member(&mut self, tag: u8, class: &str, name: &str, desc: &str) {
        let c_idx = self.class(class);
        let nat_idx = self.nat(name, desc);
        let key = format!("{class}\0{name}\0{desc}");
        self.add(tag, key, [u2(c_idx), u2(nat_idx)].concat());
    }

    pub(crate) fn method_ref(mut self, class: &str, name: &str, desc: &str) -> Self {
        self.member(TAG_METHODREF, class, name, desc);
        self
    }

    pub(crate) fn interface_method_ref(mut self, class: &str, name: &str, desc: &str) -> Self {
        self.member(TAG_IFACEMETHODREF, class, name, desc);
        self
    }

    pub(crate) fn field_ref(mut self, class: &str, name: &str, desc: &str) -> Self {
        self.member(TAG_FIELDREF, class, name, desc);
        self
    }

    /// Добавляет класс в implements.
    pub(crate) fn interface(mut self, name: &str) -> Self {
        self.interfaces.push(name.to_string());
        self
    }

    /// Объявляет метод с именем и дескриптором (без кода).
    pub(crate) fn method(mut self, name: &str, desc: &str) -> Self {
        self.methods.push((0x0001, name.to_string(), desc.to_string()));
        self
    }

    /// Объявляет native-метод (флаг ACC_NATIVE).
    pub(crate) fn native_method(mut self, name: &str, desc: &str) -> Self {
        self.methods.push((0x0101, name.to_string(), desc.to_string()));
        self
    }

    /// Сериализует class-файл.
    pub(crate) fn build(mut self) -> Vec<u8> {
        let this = self.class(&self.name.clone());
        let super_idx = self.class(&self.super_class.clone());
        let interfaces = self.interfaces.clone();
        let ifaces: Vec<u16> = interfaces.iter().map(|n| self.class(n)).collect();
        let methods = self.methods.clone();
        let method_info: Vec<(u16, u16, u16)> = methods
            .iter()
            .map(|(f, n, d)| (*f, self.utf8(n), self.utf8(d)))
            .collect();

        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        out.extend_from_slice(&u2(0));
        out.extend_from_slice(&u2(MAJOR));
        out.extend_from_slice(&u2(self.pool.len() as u16 + 1));
        for e in &self.pool {
            put(&mut out, e.tag, e.payload.clone());
        }
        out.extend_from_slice(&u2(0x0021)); // public super
        out.extend_from_slice(&u2(this));
        out.extend_from_slice(&u2(super_idx));
        out.extend_from_slice(&u2(ifaces.len() as u16));
        for i in ifaces {
            out.extend_from_slice(&u2(i));
        }
        out.extend_from_slice(&u2(0)); // fields_count
        out.extend_from_slice(&u2(method_info.len() as u16));
        for (flags, n, d) in method_info {
            out.extend_from_slice(&u2(flags));
            out.extend_from_slice(&u2(n));
            out.extend_from_slice(&u2(d));
            out.extend_from_slice(&u2(0)); // attributes_count
        }
        out.extend_from_slice(&u2(0)); // class attributes_count
        out
    }
}

/// Собирает jar (zip) в памяти из готовых записей.
pub(crate) fn jar(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    for (name, data) in entries {
        if let Err(e) = w.start_file(name.as_str(), opts) {
            panic!("start_file {name}: {e}");
        }
        if let Err(e) = w.write_all(data) {
            panic!("write {name}: {e}");
        }
    }
    match w.finish() {
        Ok(c) => c.into_inner(),
        Err(e) => panic!("finish jar: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Сгенерированный class обязан проходить строгий парсер cafebabe.
    #[test]
    fn minimal_class_is_valid_for_cafebabe() {
        let bytes = ClassBuilder::new("com/example/Min").build();
        assert!(cafebabe::parse_class(&bytes).is_ok());
    }

    #[test]
    fn repeated_constants_share_pool_slots() {
        let mut b = ClassBuilder::new("com/example/Dedup");
        let s1 = b.utf8("dup");
        let s2 = b.utf8("dup");
        let bytes = b.string("dup").build();
        assert_eq!(s1, s2);
        assert!(cafebabe::parse_class(&bytes).is_ok());
    }
}
