//! Разбор .class-файлов без внешней Java (крейт cafebabe).
//!
//! Сканируется constant pool: строковые константы (URL, пути, команды),
//! ссылки на методы и поля (`Runtime.exec`, `URLClassLoader` и т.п.) и имена
//! классов (mixin-цели, обфускация). Полный байткод не разбирается — для
//! статических правил сканера достаточно пула констант.


use std::collections::HashSet;

/// Максимум строковых констант, извлекаемых из одного класса: защита от
/// архивов-пустышек с миллионом констант.
const MAX_STRINGS: usize = 4096;

/// Максимум int-констант из пула (для правила отложенного запуска).
const MAX_INTS: usize = 512;

/// Ссылка на метод из constant pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MethodCall {
    /// Класс-владелец в бинарной форме: `java/lang/Runtime`.
    pub class: String,
    pub name: String,
    /// Дескриптор метода: `()Ljava/lang/Process;`.
    pub descriptor: String,
}

/// Ссылка на поле из constant pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FieldRef {
    pub class: String,
    pub name: String,
    /// Дескриптор поля: `Ljava/lang/String;`.
    pub descriptor: String,
}

/// Разобранный class-файл: только то, что нужно правилам сканера.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClassData {
    /// Бинарное имя класса: `com/example/mymod/Foo`.
    pub name: String,
    /// Реализуемые интерфейсы (бинарные имена).
    pub interfaces: Vec<String>,
    /// Уникальные строковые константы в порядке появления.
    pub strings: Vec<String>,
    /// Классы, упомянутые в пуле (ClassInfo): цели @Mixin, ссылки на
    /// Unsafe, скриптовые движки и т.п.
    pub class_refs: Vec<String>,
    /// Ссылки на методы (включая interface method ref).
    pub method_calls: Vec<MethodCall>,
    /// Ссылки на поля.
    pub field_refs: Vec<FieldRef>,
    /// Имена объявленных методов — для эвристики обфускации (a/b/c).
    pub declared_methods: Vec<String>,
    /// Число методов с флагом native (JNI).
    pub native_methods: usize,
    /// Int-константы пула (например, аргументы Thread.sleep).
    pub int_constants: Vec<i32>,
}

/// Разбирает один .class. `None`, если файл битый или это не class-файл.
pub(crate) fn parse(bytes: &[u8]) -> Option<ClassData> {
    let cf = cafebabe::parse_class(bytes).ok()?;
    let mut data = ClassData {
        name: cf.this_class.to_string(),
        interfaces: cf.interfaces.iter().map(|s| s.to_string()).collect(),
        ..ClassData::default()
    };

    let mut seen_strings: HashSet<String> = HashSet::new();
    let mut push_string = |s: &str, out: &mut Vec<String>| {
        if seen_strings.insert(s.to_string()) {
            out.push(s.to_string());
        }
    };

    for item in cf.constantpool_iter() {
        use cafebabe::constant_pool::{ConstantPoolItem, LiteralConstant};
        match item {
            ConstantPoolItem::ClassInfo(name) => {
                push_string(&name, &mut data.class_refs);
            }
            ConstantPoolItem::LiteralConstant(LiteralConstant::String(s)) => {
                if data.strings.len() < MAX_STRINGS {
                    push_string(&s, &mut data.strings);
                }
            }
            ConstantPoolItem::LiteralConstant(LiteralConstant::Integer(v)) => {
                if data.int_constants.len() < MAX_INTS {
                    data.int_constants.push(v);
                }
            }
            ConstantPoolItem::MethodRef(r) | ConstantPoolItem::InterfaceMethodRef(r) => {
                data.method_calls.push(MethodCall {
                    class: r.class_name.to_string(),
                    name: r.name_and_type.name.to_string(),
                    descriptor: r.name_and_type.descriptor.to_string(),
                });
            }
            ConstantPoolItem::FieldRef(r) => {
                data.field_refs.push(FieldRef {
                    class: r.class_name.to_string(),
                    name: r.name_and_type.name.to_string(),
                    descriptor: r.name_and_type.descriptor.to_string(),
                });
            }
            _ => {}
        }
    }

    for m in &cf.methods {
        if m.access_flags.contains(cafebabe::MethodAccessFlags::NATIVE) {
            data.native_methods += 1;
        }
        data.declared_methods.push(m.name.to_string());
    }
    Some(data)
}

#[cfg(test)]
mod tests {
    use crate::scanner::synthetic::{jar, ClassBuilder};
    use super::*;

    #[test]
    fn parses_strings_and_method_refs() {
        let bytes = ClassBuilder::new("com/example/mymod/Foo")
            .string("https://discord.com/api/webhooks/123/abc")
            .method_ref("java/lang/Runtime", "exec", "(Ljava/lang/String;)Ljava/lang/Process;")
            .build();
        let d = match parse(&bytes) {
            Some(d) => d,
            None => panic!("синтетический class должен разбираться"),
        };
        assert_eq!(d.name, "com/example/mymod/Foo");
        assert!(d
            .strings
            .iter()
            .any(|s| s.contains("discord.com/api/webhooks")));
        assert_eq!(d.method_calls.len(), 1);
        assert_eq!(d.method_calls[0].class, "java/lang/Runtime");
        assert_eq!(d.method_calls[0].name, "exec");
    }

    #[test]
    fn field_refs_and_interfaces() {
        let bytes = ClassBuilder::new("com/example/Bar")
            .field_ref("java/lang/System", "out", "Ljava/io/PrintStream;")
            .interface("org/spongepowered/asm/mixin/Mixin")
            .build();
        let d = parse(&bytes).expect("class");
        assert_eq!(d.field_refs.len(), 1);
        assert_eq!(d.field_refs[0].name, "out");
        assert_eq!(d.interfaces, vec!["org/spongepowered/asm/mixin/Mixin"]);
    }

    #[test]
    fn declared_methods_are_collected() {
        let bytes = ClassBuilder::new("com/example/Baz")
            .method("a", "()V")
            .method("onInitialize", "()V")
            .build();
        let d = parse(&bytes).expect("class");
        assert_eq!(d.declared_methods, vec!["a", "onInitialize"]);
    }

    #[test]
    fn duplicate_strings_deduplicated() {
        let bytes = ClassBuilder::new("com/example/Dup")
            .string("same")
            .string("same")
            .string("other")
            .build();
        let d = parse(&bytes).expect("class");
        assert_eq!(d.strings, vec!["same", "other"]);
    }

    #[test]
    fn interface_method_refs_count_as_calls() {
        let bytes = ClassBuilder::new("com/example/Iface")
            .interface_method_ref(
                "javax/script/Invocable",
                "invokeFunction",
                "(Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/Object;",
            )
            .build();
        let d = parse(&bytes).expect("class");
        assert_eq!(d.method_calls.len(), 1);
        assert_eq!(d.method_calls[0].class, "javax/script/Invocable");
        assert_eq!(d.method_calls[0].name, "invokeFunction");
    }

    #[test]
    fn native_methods_and_int_constants() {
        let bytes = ClassBuilder::new("com/example/Nat")
            .native_method("doThings", "()V")
            .int_constant(3_600_000)
            .build();
        let d = parse(&bytes).expect("class");
        assert_eq!(d.native_methods, 1);
        assert_eq!(d.int_constants, vec![3_600_000]);
    }

    #[test]
    fn class_refs_are_collected() {
        // @Mixin(Target.class) хранит цель как ClassInfo, а не строку.
        let bytes = ClassBuilder::new("com/example/MixinSession")
            .interface("org/spongepowered/asm/mixin/Mixin")
            .method_ref(
                "net/minecraft/util/Session",
                "getToken",
                "()Ljava/lang/String;",
            )
            .build();
        let d = parse(&bytes).expect("class");
        assert!(d.class_refs.iter().any(|c| c == "net/minecraft/util/Session"));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(parse(b"not a class file at all").is_none());
        assert!(parse(b"").is_none());
        assert!(parse(&[0xCA, 0xFE, 0xBA, 0xBE, 9, 9, 9, 9]).is_none());
    }

    #[test]
    fn jar_round_trip_keeps_entries() {
        let cls = ClassBuilder::new("com/example/Foo").string("hello").build();
        let meta = br#"{"id":"foo","version":"1.0.0","name":"Foo"}"#;
        let bytes = jar(&[
            ("fabric.mod.json".to_string(), meta.to_vec()),
            ("com/example/Foo.class".to_string(), cls),
        ]);
        let cursor = std::io::Cursor::new(&bytes);
        let mut z = match zip::ZipArchive::new(cursor) {
            Ok(z) => z,
            Err(e) => panic!("jar: {e}"),
        };
        assert!(z.by_name("fabric.mod.json").is_ok());
        assert!(z.by_name("com/example/Foo.class").is_ok());
        assert_eq!(z.len(), 2);
    }
}
