//! Конфиг сканера: все веса правил и пороги уровней в одном файле
//! (`<data>/scanner.json`). Режимы проверки (обычный/усиленный/параноидальный)
//! меняют только пороги и бонусы, набор правил один и тот же.


use serde::{Deserialize, Serialize};

/// Веса правил: сколько находка правила добавляет к score 0–100.
/// Отрицательные значения запрещены — правила только штрафуют.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Weights {
    /// R1: каналы вывода данных (webhook Discord, Telegram bot, pastebin…).
    pub exfil_channel: i32,
    /// R2: кража данных (файлы аккаунтов, браузеры, кошельки, скриншоты).
    pub data_theft: i32,
    /// R2b: доступ к токену/сессии (getAccessToken/getSession).
    pub session_token_access: i32,
    /// R3: запуск процессов (ProcessBuilder, Runtime.exec, powershell…).
    /// Сам по себе exec мягкий; hard-статус даёт только комбинация
    /// с записью файлов/скачиванием/подозрительными командами.
    pub process_exec: i32,
    /// R4: динамическая загрузка кода (URLClassLoader, defineClass).
    pub dynamic_code_load: i32,
    /// R4b: обращение к sun.misc.Unsafe (приём оптимизаторов, мягкий вес).
    pub unsafe_use: i32,
    /// R5: URL исполняемого файла + запись на диск + запуск.
    pub executable_download: i32,
    /// R5b: только URL на .exe/.dll/.bat/.ps1 (без комбо).
    pub executable_download_url_only: i32,
    /// R6: нативные библиотеки в jar (.dll/.so).
    pub native_binaries: i32,
    /// R6: `..` в путях записей архива.
    pub path_traversal: i32,
    /// R6: битые class-файлы.
    pub broken_class: i32,
    /// R6: классы вне пакета modid.
    pub classes_outside_modid: i32,
    /// R6: jar-in-jar в META-INF/jars (легитимен, малый вес).
    pub jar_in_jar: i32,
    /// R6: превышены лимиты архива (подозрение на zip-бомбу).
    pub archive_limits: i32,
    /// R7: сильная обфускация (имена a/b/c, энтропия строк).
    pub obfuscation: i32,
    /// R8: закодированные данные без находок при декодировании.
    pub encoded_payload: i32,
    /// R9: обращения к доменам вне белого списка и метаданных мода.
    pub unknown_domain: i32,
    /// R11: SHA-256 в локальной базе известной малвари.
    pub known_malware_hash: i32,
    /// R11: строковый индикатор из базы малвари.
    pub malware_indicator: i32,
    /// R12: Premain-Class / Agent-Class / Launcher-Agent-Class.
    pub manifest_agent: i32,
    /// R12: подозрительный Class-Path (URL, абсолютные пути).
    pub manifest_classpath: i32,
    /// R13: запись в чужие jar в mods/.
    pub infect_other_mods: i32,
    /// R14: System.load / loadLibrary.
    pub system_load: i32,
    /// R14b: native-методы (JNI).
    pub jni_native_methods: i32,
    /// R15: скриптовые движки (ScriptEngine, Nashorn, Groovy, Rhino).
    pub script_engine: i32,
    /// R16: фингерпринт системы (getenv, user.name, tasklist…).
    pub fingerprinting: i32,    /// R17: отложенный запуск (даты, большие Thread.sleep).
    pub delayed_execution: i32,
    /// R18: проверки VM/дебаггера.
    pub anti_analysis: i32,
    /// R18b: addShutdownHook.
    pub shutdown_hook: i32,
    /// R19: массовый setAccessible(true).
    pub mass_set_accessible: i32,
    /// R20: mixin на классы сессии, аккаунтов и сети.
    pub session_mixin: i32,
    /// R21: дубли классов между модами.
    pub duplicate_classes: i32,
    /// R21b: перекрытие классов Minecraft/загрузчика.
    pub override_mc_loader: i32,
    /// R22: нет метаданных мода.
    pub no_metadata: i32,
    /// R22b: modid не совпадает с пакетами или метаданные без описания.
    pub bare_metadata: i32,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            exfil_channel: 45,
            data_theft: 40,
            session_token_access: 20,
            process_exec: 18,
            dynamic_code_load: 30,
            unsafe_use: 10,
            executable_download: 60,
            executable_download_url_only: 35,
            native_binaries: 20,
            path_traversal: 50,
            broken_class: 12,
            classes_outside_modid: 10,
            jar_in_jar: 3,
            archive_limits: 40,
            obfuscation: 45,
            encoded_payload: 4,
            unknown_domain: 6,
            known_malware_hash: 100,
            malware_indicator: 50,
            manifest_agent: 45,
            manifest_classpath: 15,
            infect_other_mods: 50,
            system_load: 18,
            jni_native_methods: 8,
            script_engine: 22,
            fingerprinting: 5,
            delayed_execution: 14,
            anti_analysis: 25,
            shutdown_hook: 8,
            mass_set_accessible: 15,
            session_mixin: 28,
            duplicate_classes: 16,
            override_mc_loader: 45,
            no_metadata: 25,
            bare_metadata: 6,
        }
    }
}

/// Пороги одного режима: уровни и бонусы/штрафы за хеш на Modrinth.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ModeThresholds {
    /// score >= danger — уровень danger.
    pub danger: i32,
    /// score >= suspicious — уровень suspicious.
    pub suspicious: i32,
    /// Бонус за найденный на Modrinth/CurseForge хеш (вычитается из score,
    /// но не ниже суммы hard-находок).
    pub modrinth_known_bonus: i32,
    /// Штраф за хеш, которого нет в каталогах.
    pub modrinth_unknown_penalty: i32,
}

impl ModeThresholds {
    fn new(danger: i32, suspicious: i32, known: i32, unknown: i32) -> Self {
        Self {
            danger,
            suspicious,
            modrinth_known_bonus: known,
            modrinth_unknown_penalty: unknown,
        }
    }

    fn sanitize(&mut self) {
        self.danger = self.danger.clamp(1, 100);
        self.suspicious = self.suspicious.clamp(1, 100);
        if self.suspicious > self.danger {
            self.suspicious = self.danger;
        }
        self.modrinth_known_bonus = self.modrinth_known_bonus.clamp(0, 100);
        self.modrinth_unknown_penalty = self.modrinth_unknown_penalty.clamp(0, 100);
    }
}

/// Пороги всех трёх режимов. Режимы отличаются только этими числами
/// (и списками «блок при любой находке» — они фиксированы ТЗ).
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Thresholds {
    pub normal: ModeThresholds,
    pub strict: ModeThresholds,
    pub paranoid: ModeThresholds,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            // Обычный: блок только при danger (>=70), Modrinth даёт −50.
            normal: ModeThresholds::new(70, 35, 50, 0),
            // Усиленный: danger при 45, Modrinth −25, неизвестный хеш +20.
            strict: ModeThresholds::new(45, 35, 25, 20),
            // Параноидальный: подозрительно всё, кроме чистого известного мода.
            paranoid: ModeThresholds::new(1, 1, 0, 100),
        }
    }
}

/// Лимиты сканера: защита от zip-бомб и архивов-пустышек.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Limits {
    /// Максимальный размер jar, который вообще сканируем.
    pub max_jar_bytes: u64,
    /// Максимальное число записей в архиве.
    pub max_entries: usize,
    /// Максимальная суммарная распакованная глубина (по центральному каталогу).
    pub max_uncompressed_bytes: u64,
    /// Максимальный размер одного class-файла, который читаем.
    pub max_class_bytes: u64,
    /// Максимум class-файлов, разбираемых в одном jar.
    pub max_classes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_jar_bytes: 1024 * 1024 * 1024,
            max_entries: 50_000,
            max_uncompressed_bytes: 8 * 1024 * 1024 * 1024,
            max_class_bytes: 16 * 1024 * 1024,
            max_classes: 30_000,
        }
    }
}

/// Режим проверки: обычный | усиленный | параноидальный.
/// Веса правил одинаковы, режим меняет пороги и поведение при запуске.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ScanMode {
    #[default]
    Normal,
    Strict,
    Paranoid,
}

impl ScanMode {
    /// Пороги этого режима из конфига.
    pub(crate) fn thresholds(self, cfg: &ScannerConfig) -> ModeThresholds {
        match self {
            ScanMode::Normal => cfg.thresholds.normal,
            ScanMode::Strict => cfg.thresholds.strict,
            ScanMode::Paranoid => cfg.thresholds.paranoid,
        }
    }
}

/// Корневой конфиг сканера (`scanner.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ScannerConfig {
    pub weights: Weights,
    pub thresholds: Thresholds,
    pub limits: Limits,
    /// Домены, обращения к которым не считаются подозрительными.
    pub whitelist_domains: Vec<String>,
}

impl ScannerConfig {
    /// Конфиг с доменным белым списком по умолчанию.
    pub(crate) fn with_default_whitelist() -> Self {
        let whitelist = [
            "modrinth.com",
            "www.modrinth.com",
            "api.modrinth.com",
            "cdn.modrinth.com",
            "api.curseforge.com",
            "edge.forgecdn.net",
            "mediafilez.forgecdn.net",
            "meta.fabricmc.net",
            "maven.fabricmc.net",
            "maven.quiltmc.org",
            "maven.neoforged.net",
            "meta.minecraftforge.net",
            "maven.minecraftforge.net",
            "libraries.minecraft.net",
            "piston-meta.mojang.com",
            "piston-data.mojang.com",
            "launchermeta.mojang.com",
            "api.mojang.com",
            "sessionserver.mojang.com",
            "textures.minecraft.net",
            "resources.download.minecraft.net",
            "crafatar.com",
            "api.ashcon.app",
            "mc-heads.net",
            "minotar.net",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        Self {
            whitelist_domains: whitelist,
            ..Self::default()
        }
    }

    /// Приводит значения из файла/IPC к безопасным диапазонам.
    pub(crate) fn sanitize(&mut self) {
        let w = &mut self.weights;
        for v in [
            &mut w.exfil_channel,
            &mut w.data_theft,
            &mut w.session_token_access,
            &mut w.process_exec,
            &mut w.dynamic_code_load,
            &mut w.unsafe_use,
            &mut w.executable_download,
            &mut w.executable_download_url_only,
            &mut w.native_binaries,
            &mut w.path_traversal,
            &mut w.broken_class,
            &mut w.classes_outside_modid,
            &mut w.jar_in_jar,
            &mut w.archive_limits,
            &mut w.obfuscation,
            &mut w.encoded_payload,
            &mut w.unknown_domain,
            &mut w.known_malware_hash,
            &mut w.malware_indicator,
            &mut w.manifest_agent,
            &mut w.manifest_classpath,
            &mut w.infect_other_mods,
            &mut w.system_load,
            &mut w.jni_native_methods,
            &mut w.script_engine,
            &mut w.fingerprinting,
            &mut w.delayed_execution,
            &mut w.anti_analysis,
            &mut w.shutdown_hook,
            &mut w.mass_set_accessible,
            &mut w.session_mixin,
            &mut w.duplicate_classes,
            &mut w.override_mc_loader,
            &mut w.no_metadata,
            &mut w.bare_metadata,
        ] {
            *v = (*v).clamp(0, 100);
        }
        self.thresholds.normal.sanitize();
        self.thresholds.strict.sanitize();
        self.thresholds.paranoid.sanitize();
        self.limits.max_jar_bytes = self.limits.max_jar_bytes.clamp(1024 * 1024, 8 * 1024 * 1024 * 1024);
        self.limits.max_entries = self.limits.max_entries.clamp(100, 500_000);
        self.limits.max_uncompressed_bytes =
            self.limits.max_uncompressed_bytes.clamp(1024 * 1024, 64 * 1024 * 1024 * 1024);
        self.limits.max_class_bytes = self.limits.max_class_bytes.clamp(64 * 1024, 128 * 1024 * 1024);
        self.limits.max_classes = self.limits.max_classes.clamp(10, 200_000);
        self.whitelist_domains.retain(|d| !d.is_empty() && d.len() <= 253 && !d.contains(' '));
        self.whitelist_domains.truncate(500);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let mut cfg = ScannerConfig::with_default_whitelist();
        cfg.sanitize();
        let t = cfg.thresholds.normal;
        assert!(t.suspicious < t.danger);
        assert_eq!(cfg.thresholds.strict.danger, 45);
        assert_eq!(cfg.thresholds.paranoid.modrinth_unknown_penalty, 100);
        assert!(cfg.weights.known_malware_hash >= 100);
        assert!(cfg.whitelist_domains.contains(&"api.modrinth.com".to_string()));
    }

    #[test]
    fn sanitize_clamps_weights_and_thresholds() {
        let mut cfg = ScannerConfig {
            weights: Weights {
                exfil_channel: 999,
                jar_in_jar: -5,
                ..Weights::default()
            },
            thresholds: Thresholds {
                normal: ModeThresholds::new(0, 200, 10, 10),
                ..Thresholds::default()
            },
            whitelist_domains: vec!["".into(), "ok.example".into()],
            ..ScannerConfig::default()
        };
        cfg.sanitize();
        assert_eq!(cfg.weights.exfil_channel, 100);
        assert_eq!(cfg.weights.jar_in_jar, 0);
        assert_eq!(cfg.thresholds.normal.danger, 1);
        assert_eq!(cfg.thresholds.normal.suspicious, 1);
        assert_eq!(cfg.whitelist_domains, vec!["ok.example".to_string()]);
    }

    #[test]
    fn config_round_trips_camel_case() {
        let cfg = ScannerConfig::with_default_whitelist();
        let raw = serde_json::to_string(&cfg).expect("serialize");
        assert!(raw.contains("\"exfilChannel\""));
        assert!(raw.contains("\"modrinthKnownBonus\""));
        let back: ScannerConfig = serde_json::from_str(&raw).expect("deserialize");
        assert_eq!(back.weights.exfil_channel, cfg.weights.exfil_channel);
        assert_eq!(back.thresholds.strict.danger, cfg.thresholds.strict.danger);
    }
}
