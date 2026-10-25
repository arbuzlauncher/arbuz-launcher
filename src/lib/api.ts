// Типы данных и обёртки над Tauri-командами
import { invoke } from '@tauri-apps/api/core';
import type { CustomTheme } from './theme';

/** Корень данных лаунчера: папка `arbuz-launcher` рядом с лаунчером
 *  (или в профиле пользователя) — путь считает Rust, чтобы он совпадал. */
export const dataRoot = (): Promise<string> => invoke<string>('data_dir');

export type Loader = 'vanilla' | 'fabric' | 'forge' | 'neoforge' | 'quilt';
export type AccountKind = 'microsoft' | 'elyby' | 'littleskin' | 'offline';

export interface Instance {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion: string | null;
  biome: string;
  icon: string | null;
  /** id встроенной иконки профиля (src/assets/profiles). */
  iconPreset: string | null;
  /** id встроенных обоев профиля (public/builtin/wallpapers). */
  wallpaper: string | null;
  ramMinMb: number;
  ramMaxMb: number;
  javaMajor: number;
  javaPath: string | null;
  jvmArgs: string;
  width: number;
  height: number;
  fullscreen: boolean;
  playtimeSec: number;
  playtimeDays: Record<string, number>;
  lastPlayed: number | null;
  created: number;
  /** Сборка, из которой создан профиль (для обновлений); у старых профилей нет. */
  pack?: PackRef | null;
  /** Своя папка игры (`.minecraft`); null — `instances/<id>/minecraft`. */
  gameDir?: string | null;
  /** Режим сканера модов для профиля: 'normal' | 'strict' | 'paranoid' | null (глобальный). */
  scannerMode: string | null;
}

/** Источник сборки профиля. */
export interface PackRef {
  source: string;
  projectId: string;
  versionId: string;
  versionName: string;
}

/** Доступное обновление сборки. */
export interface PackUpdate {
  versionId: string;
  versionName: string;
  changelog: string;
  date: string;
}

export interface Account {
  id: string;
  kind: AccountKind;
  name: string;
  note: string;
}

/** Игрок в P2P-комнате. `slot` 0 — хозяин комнаты. */
export interface RoomPlayer {
  slot: number;
  name: string;
}

/** Состояние комнаты: одинаковое у всех, кроме `me`. */
export interface Room {
  players: RoomPlayer[];
  /** Слот владельца: может выгонять и менять код. */
  admin: number;
  /** Мой слот (0 — я хозяин). */
  me: number;
  kicked: boolean;
  host: string;
}

/** Новый релиз лаунчера на GitHub. */
export interface LauncherUpdate {
  version: string;
  current: string;
  page: string;
  download: string;
  notes: string;
}

export interface AnimConfig {
  enabled: boolean;
  preset: string;
  speed: number;
  amplitude: number;
  stagger: number;
  easing: string;
  curve: string;
  /** Точки своей кривой: [[x, y], ...], x 0..1, y может выходить за 0..1. */
  points: [number, number][];
  dir: string;
  screens: boolean;
  lists: boolean;
  modals: boolean;
  press: boolean;
  launch: boolean;
}

export interface Config {
  accent: string;
  dark: boolean;
  style: string;
  /** Язык интерфейса: ru | en | pl | uk | eo. */
  lang: string;
  /** Показывать выбор тем (экспериментально). */
  experimentalThemes: boolean;
  /** Свои обои главной: имя файла в папке wallpapers. */
  homeWallpaper: string | null;
  /** Визард первого запуска пройден. */
  onboarded: boolean;
  activeAccount: string | null;
  defaultJava: number;
  closeOnLaunch: boolean;
  showConsole: boolean;
  /** Проверять обновления лаунчера на GitHub при запуске. */
  checkLauncherUpdates: boolean;
  /** Закрытие окна прячет лаунчер в трей. */
  closeToTray: boolean;
  hideEmpty: boolean;
  modIsolationEnabled: boolean;
  modIsolationLevel: string;
  /** Сканер модов перед запуском игры. */
  scannerEnabled: boolean;
  /** Режим сканера: 'normal' | 'strict' | 'paranoid'. */
  scannerMode: string;
  /** ИИ-этап сканера (заметки по подозрительным модам). */
  aiScanner: boolean;
  /** URL статичного JSON со списком серверов. */
  serversUrl: string;
  /** Показывать статистику времени игры. */
  showPlaytimeStats: boolean;
  curseforgeApiKey: string;
  /** Экспериментально: ИИ-ассистент сборок по промпту. */
  aiBuilder: boolean;
  /** Лимит модов для ИИ-сборки: 0 — без ограничения. */
  aiPackLimit: number;
  /** Приоритетный каталог ИИ-сборки: пусто — оба сразу. */
  aiPackSource: string;
  /** Модель ИИ-сборки: пусто — первая доступная бесплатная. */
  aiPackModel: string;
  anim: AnimConfig;
  custom: CustomTheme;
  moves: Record<string, [number, number]>;
  sync: SyncSettings | null;
}

export interface JavaInstall {
  major: number;
  version: string;
  path: string;
}

export interface McVersion {
  id: string;
  kind: string;
  releaseTime: string;
}

/** Установленный контент профиля (ТЗ §9). */
export interface ContentItem {
  /** Относительный путь от `minecraft/` без суффикса `.disabled`. */
  file: string;
  name: string;
  version: string;
  /** mod | rp | sh | dp */
  kind: string;
  sizeBytes: number;
  enabled: boolean;
  sha1: string;
  projectId: string | null;
  /** Источник, из которого файл поставлен через каталог. */
  source: string | null;
  /** Абсолютный путь к иконке в кэше — нужен `convertFileSrc`. */
  icon: string | null;
  /** Иконка проекта на CDN, если своей в архиве нет. */
  iconUrl: string | null;
  /** Помечен ли файл как изолируемый (песочница при запуске). */
  isolated: boolean;
}

export interface World {
  dir: string;
  name: string;
  mode: string;
  lastPlayedMs: number;
}

/** Скриншот из `minecraft/screenshots`. */
export interface Shot {
  file: string;
  size: number;
  /** unix-время изменения, мс. */
  modified: number;
}

/** Вид контента в каталоге (ТЗ §10). */
export type CatalogKind = 'mod' | 'dp' | 'rp' | 'pk' | 'sh';
export type CatalogSource = 'modrinth' | 'curseforge';

/** Аргументы поиска по каталогу (совпадают с `catalog::SearchArgs`). */
export interface SearchArgs {
  source: string;
  kind: string;
  query: string;
  loaders: string[];
  mcVersion: string | null;
  categories: string[];
  offset: number;
  sort: string;
  instanceId: string | null;
}

/** Элемент каталога (совпадает с `catalog::CatalogItem`). */
export interface CatalogItem {
  id: string;
  title: string;
  author: string;
  description: string;
  downloads: number;
  loaders: string[];
  categories: string[];
  iconUrl: string | null;
  source: string;
  kind: string;
  installed: boolean;
}

export interface CatalogSearch {
  hits: CatalogItem[];
  total: number;
}

/** Ссылка проекта на странице мода. */
export interface ProjectLink {
  label: string;
  url: string;
}

/** Подробности проекта для страницы мода. */
export interface ProjectDetails {
  id: string;
  source: string;
  title: string;
  description: string;
  body: string;
  downloads: number;
  iconUrl: string | null;
  links: ProjectLink[];
}

/** Разбор падения игры для окна «Игра упала». */
export interface CrashDiagnosis {
  found: boolean;
  source: string;
  file: string | null;
  kind: string;
  title: string;
  summary: string;
  culprit: string | null;
  culpritFile: string | null;
  suspects: string[];
  hints: string[];
  excerpt: string[];
  /** Ключи для t(): локализованный заголовок и описание. */
  titleKey: string | null;
  summaryKey: string | null;
  hintKeys: string[];
  /** Подстановки для ключей: mod, frame, error, mods. */
  vars: Record<string, string>;
}

/** Ответ ИИ-разбора падения (free-модель OpenCode Zen). */
export interface AiDiagnosis {
  model: string;
  cause: string;
  modName: string | null;
  confidence: number;
  reason: string;
  actions: string[];
  elapsedMs: number;
  tokens: number | null;
}

/** Проект каталога, подобранный ИИ-ассистентом сборки. */
export interface PlanItem {
  /** mod | rp | sh */
  kind: string;
  source: string;
  id: string;
  title: string;
  author: string;
  description: string;
  iconUrl: string | null;
  downloads: number;
  /** Зачем проект в сборке. */
  why: string;
}

/** План сборки: что ассистент предлагает поставить. */
export interface PackPlan {
  model: string;
  name: string;
  summary: string;
  description: string;
  mcVersion: string;
  loader: string;
  mods: PlanItem[];
  resourcepacks: PlanItem[];
  shaders: PlanItem[];
  /** Проекты, которые не нашлись в каталоге. */
  skipped: string[];
}

/** Доступ к игре по сети: проброшенный порт и внешний адрес. */
export interface LanShare {
  port: number;
  /** Внешний адрес; пусто, если роутер его не сообщил. */
  host: string;
  /** Адрес внутри сети: `192.168.0.5:25565`. */
  local: string;
}

/** P2P-канал: код для друга и адрес для Minecraft у подключающегося. */
export interface P2pShare {
  /** Код, который передаём другу. */
  code: string;
  /** Адрес для Minecraft: `127.0.0.1:25565`. */
  local: string;
  port: number;
  /** Мы принимаем (`true`) или подключаемся (`false`). */
  hosting: boolean;
  /** Комната создана, но мир ещё не открыт для сети. */
  waiting: boolean;
}

/** Профиль Modrinth App, доступный для импорта. */
export interface ModrinthProfile {
  name: string;
  path: string;
  mcVersion: string;
  loader: string;
  loaderVersion: string | null;
  mods: number;
  hasIcon: boolean;
}

/** Манифест сборки (`pack.json` в папке профиля). */
export interface PackManifest {
  name: string;
  version: string;
  author: string;
  summary: string;
  description: string;
  icon: string | null;
}

/** Сводка по сборке для вкладки «Сборка». */
export interface PackInfo {
  id: string;
  name: string;
  manifest: PackManifest;
  mcVersion: string;
  loader: string;
  loaderVersion: string | null;
  mods: number;
  modsTotal: number;
  configs: number;
  sizeBytes: number;
  icon: string | null;
}

/** Файл конфигурации мода. */
export interface ConfigEntry {
  path: string;
  size: number;
  kind: string;
  modId: string | null;
  modName: string | null;
}

/** Открытый конфиг для редактора. */
export interface ConfigDoc {
  path: string;
  content: string;
  size: number;
  hasBackup: boolean;
}

/** Замечание проверки сборки. */
export interface PackIssue {
  severity: 'error' | 'warn' | 'info' | string;
  title: string;
  detail: string;
}

/** Результат экспорта сборки. */
export interface PackExport {
  path: string;
  files: number;
  bytes: number;
}

/** Доступное обновление установленного контента. */
export interface UpdateInfo {
  file: string;
  name: string;
  newVersion: string;
}
/** Новость Minecraft с «Главной» (ТЗ §12-b). */
export interface NewsItem {
  title: string;
  version: string;
  /** ISO-8601. */
  date: string;
  imageUrl: string | null;
}

/** ИИ-заметка сканера по одному моду (пояснение, не приговор). */
export interface AiNote {
  model: string;
  assessment: string;
  confidence: number;
  note: string;
  elapsedMs: number;
}

/** Сервер из каталога (статичный JSON по URL из настроек). */
export interface CatalogServer {
  name: string;
  ip: string;
  version: string;
  mode: string;
}

/** Результат Server List Ping. */
export interface PingInfo {
  host: string;
  port: number;
  online: boolean;
  latencyMs: number;
  version: string;
  playersOnline: number;
  playersMax: number;
  motd: string;
}

/** Прогон бенчмарка сборки. */
export interface BenchRun {
  date: number;
  mcVersion: string;
  loader: string;
  mods: number;
  world: string | null;
  /** мс до готовности игры (меню/атласы); 0 — маркер не увидели. */
  loadMs: number;
  ramAvgMb: number;
  ramPeakMb: number;
  cpuAvg: number;
  cpuPeak: number;
  samples: number;
}

/** Снимок ресурсов процесса игры для монитора. */
export interface MonitorSample {
  pid: number;
  found: boolean;
  ramMb: number;
  cpuPct: number;
}

/** Закреплённый на профиле сервер. */
export interface PinnedServer {
  name: string;
  ip: string;
}

/** Владелец класса из стектрейса (мод из mods/). */
export interface ClassOwner {
  name: string;
  file: string;
}

/** Одна находка сканера модов. */
export interface ScanFinding {
  rule: string;
  /** Машиночитаемый код: r6_native, r12_agent… */
  code: string;
  weight: number;
  hard: boolean;
  place: string;
  what: string;
}

/** Отчёт сканера по одному файлу контента. */
export interface ModReport {
  file: string;
  name: string;
  sha256: string;
  score: number;
  level: 'ok' | 'suspicious' | 'danger' | string;
  findings: ScanFinding[];
  knownHash: boolean;
  modrinthProject: string | null;
  offlineLookup: boolean;
  /** Файл в ручных исключениях профиля. */
  allowed: boolean;
}

/** Настройки синхронизации профилей (совпадает с sync::SyncSettings). */
export interface SyncSettings {
  /** id главного профиля. */
  master: string;
  /** Выбранные ключи options.txt. */
  keys: string[];
  /** Переносить servers.dat. */
  serversDat: boolean;
}

/** Текстура скина аккаунта: data-URL и её размеры в пикселях. */
export interface Skin {
  url: string;
  width: number;
  height: number;
}

/** Версия сборки, доступная к установке. */
export interface PackVersion {
  id: string;
  name: string;
  mcVersion: string;
  loader: string;
  /** ISO-8601. */
  date: string;
}

/** Ответ device-code Microsoft (поля как в структуре Rust — snake_case). */
export interface DeviceCode {
  device_code: string;
  user_code: string;
  verification_uri: string;
  /** Ссылка с `otc` — страница входа подставляет код сама. */
  verification_url: string;
  expires_in: number;
  interval: number;
}

export const api = {
  getConfig: () => invoke<Config>('get_config'),
  setConfig: (cfg: Config) => invoke<void>('set_config', { cfg }),
  exportSettings: (path: string) => invoke<void>('export_settings', { path }),
  importSettings: (path: string) => invoke<Config>('import_settings', { path }),

  listInstances: () => invoke<Instance[]>('list_instances'),
  createInstance: (name: string, mcVersion: string | null, loader: Loader) =>
    invoke<Instance>('create_instance', { name, mcVersion, loader }),
  updateInstance: (instance: Instance) => invoke<void>('update_instance', { instance }),
  deleteInstance: (id: string) => invoke<void>('delete_instance', { id }),

  setInstanceIcon: (id: string, sourcePath: string) =>
    invoke<Instance>('set_instance_icon', { id, sourcePath }),
  setInstanceIconUrl: (id: string, url: string) =>
    invoke<Instance>('set_instance_icon_url', { id, url }),
  clearInstanceIcon: (id: string) => invoke<Instance>('clear_instance_icon', { id }),
  /** Своя папка игры: null — вернуть папку профиля. */
  setGameDir: (id: string, path: string | null) =>
    invoke<Instance>('set_instance_game_dir', { id, path }),

  openInstanceFolder: (id: string) => invoke<void>('open_instance_folder', { id }),

  /** Хвост `logs/latest.log` — консоль игры после перезапуска лаунчера. */
  readGameLog: (id: string, tail?: number) => invoke<string[]>('read_game_log', { id, tail }),
  openLogsFolder: (id: string) => invoke<void>('open_logs_folder', { id }),

  getSystemRamMb: () => invoke<number>('get_system_ram_mb'),

  listMcVersions: () => invoke<McVersion[]>('list_mc_versions'),
  addCustomVersion: (id: string, base: string, jsonPath: string | null, jarPath: string | null) =>
    invoke<string>('add_custom_version', { id, base, jsonPath, jarPath }),
  removeCustomVersion: (id: string) => invoke<void>('remove_custom_version', { id }),
  listLoaderVersions: (loader: Loader, mcVersion: string) =>
    invoke<string[]>('list_loader_versions', { loader, mcVersion }),
  detectJavas: () => invoke<JavaInstall[]>('detect_javas'),
  installJava: (major: number) => invoke<string>('install_java', { major }),
  listAccounts: () => invoke<Account[]>('list_accounts'),
  accountStatus: (id: string) => invoke<string>('account_status', { id }),
  offlineAdd: (name: string) => invoke<Account>('offline_add', { name }),
  elybyLogin: (login: string, password: string) =>
    invoke<Account>('ely_login', { login, password }),
  littleLogin: (login: string, password: string) =>
    invoke<Account>('little_login', { login, password }),
  msBegin: () => invoke<DeviceCode>('ms_begin'),
  msPoll: (deviceCode: string, interval: number) =>
    invoke<Account>('ms_poll', { deviceCode, interval }),
  msCancel: () => invoke<void>('ms_cancel'),
  setActiveAccount: (id: string) => invoke<void>('set_active_account', { id }),
  removeAccount: (id: string) => invoke<void>('remove_account', { id }),
  launchInstance: (id: string, world: string | null, server: string | null = null, scanOverride?: boolean) =>
    invoke<void>('launch_instance', { id, world, server, scanOverride: scanOverride ?? false }),
  stopInstance: (id: string) => invoke<void>('stop_instance', { id }),

  listContent: (id: string) => invoke<ContentItem[]>('list_content', { id }),
  toggleContent: (id: string, file: string, enabled: boolean) =>
    invoke<void>('toggle_content', { id, file, enabled }),
  removeContent: (id: string, file: string) => invoke<void>('remove_content', { id, file }),
  toggleIsolation: (id: string, file: string, isolated: boolean) =>
    invoke<void>('toggle_isolation', { id, file, isolated }),
  listWorlds: (id: string) => invoke<World[]>('list_worlds', { id }),
  listScreenshots: (id: string) => invoke<Shot[]>('list_screenshots', { id }),
  openScreenshotsFolder: (id: string) => invoke<void>('open_screenshots_folder', { id }),
  openScreenshot: (id: string, file: string) => invoke<void>('open_screenshot', { id, file }),

  searchCatalog: (args: SearchArgs) => invoke<CatalogSearch>('search_catalog', { args }),
  projectDetails: (source: string, id: string) =>
    invoke<ProjectDetails>('catalog_project', { source, id }),
  installCatalogItem: (instanceId: string, source: string, projectId: string, kind: string, world: string | null) =>
    invoke<void>('install_catalog_item', { instanceId, source, projectId, kind, world }),
  checkUpdates: (id: string) => invoke<UpdateInfo[]>('check_updates', { id }),
  applyUpdates: (id: string, files: string[]) => invoke<number>('apply_updates', { id, files }),
  packUpdate: (id: string) => invoke<PackUpdate | null>('pack_update', { id }),
  applyPackUpdate: (id: string, versionId: string) =>
    invoke<Instance>('apply_pack_update', { id, versionId }),

  installModpack: (source: string, projectId: string, versionId: string | null) =>
    invoke<Instance>('install_modpack', { source, projectId, versionId }),
  listModpackVersions: (source: string, projectId: string) =>
    invoke<PackVersion[]>('list_modpack_versions', { source, projectId }),
  importPack: (path: string) => invoke<Instance>('import_pack', { path }),
  importPrismInstance: (path: string) => invoke<Instance>('import_prism_instance', { path }),
  listModrinthProfiles: () => invoke<ModrinthProfile[]>('list_modrinth_profiles'),
  importModrinthProfile: (path: string) => invoke<Instance>('import_modrinth_profile', { path }),

  diagnoseCrash: (id: string) => invoke<CrashDiagnosis>('diagnose_crash', { id }),
  aiDiagnoseCrash: (id: string, lang: string, console: string) =>
    invoke<AiDiagnosis>('ai_diagnose_crash', { id, lang, console }),
  aiPackPlan: (
    prompt: string,
    lang: string,
    mcVersion: string | null,
    loader: string | null,
    limit: number | null,
    source: string | null,
    model: string | null,
  ) => invoke<PackPlan>('ai_pack_plan', { prompt, lang, mcVersion, loader, limit, source, model }),
  aiModels: () => invoke<string[]>('ai_models'),
  openCrashReports: (id: string) => invoke<void>('open_crash_reports', { id }),

  packInfo: (id: string) => invoke<PackInfo>('pack_info', { id }),
  savePackInfo: (id: string, manifest: PackManifest) =>
    invoke<PackInfo>('save_pack_info', { id, manifest }),
  packConfigs: (id: string) => invoke<ConfigEntry[]>('pack_configs', { id }),
  readPackConfig: (id: string, path: string) =>
    invoke<ConfigDoc>('read_pack_config', { id, path }),
  savePackConfig: (id: string, path: string, content: string) =>
    invoke<ConfigDoc>('save_pack_config', { id, path, content }),
  restorePackConfig: (id: string, path: string) =>
    invoke<ConfigDoc>('restore_pack_config', { id, path }),
  validatePack: (id: string) => invoke<PackIssue[]>('validate_pack', { id }),
  exportPack: (id: string, dest: string, format: string) =>
    invoke<PackExport>('export_pack', { id, dest, format }),
  openPackConfigs: (id: string) => invoke<void>('open_pack_configs', { id }),

  accountSkin: (id: string) => invoke<Skin | null>('account_skin', { id }),

  getNews: () => invoke<NewsItem[]>('get_news'),
  openDataFolder: () => invoke<void>('open_data_folder'),
  setHomeWallpaper: (sourcePath: string) => invoke<string>('set_home_wallpaper', { sourcePath }),
  clearHomeWallpaper: () => invoke<void>('clear_home_wallpaper'),

  lanPort: (id: string) => invoke<number | null>('lan_port', { id }),
  netShare: (port: number) => invoke<LanShare>('net_share', { port }),
  netUnshare: () => invoke<void>('net_unshare'),
  netActive: () => invoke<LanShare | null>('net_active'),
  p2pHost: (port: number, name: string) => invoke<P2pShare>('p2p_host', { port, name }),
  /** Комната заранее: код можно отдать друзьям до запуска игры. */
  p2pRoom: (name: string) => invoke<P2pShare>('p2p_room', { name }),
  p2pJoin: (code: string, name: string) => invoke<P2pShare>('p2p_join', { code, name }),
  /** Кто в комнате и кто владелец. */
  p2pRoomState: () => invoke<Room | null>('p2p_room_state'),
  /** Выгнать игрока (владелец комнаты). */
  p2pKick: (slot: number) => invoke<void>('p2p_kick', { slot }),
  /** Передать права владельца (хозяин комнаты). */
  p2pAdmin: (slot: number) => invoke<void>('p2p_admin', { slot }),
  /** Новый код: старый перестаёт пускать. */
  p2pNewCode: () => invoke<P2pShare>('p2p_new_code'),
  /** Одноразовое сообщение: исключили из комнаты / связь потерялась. */
  p2pNotice: () => invoke<string | null>('p2p_notice'),
  p2pStop: () => invoke<void>('p2p_stop'),
  p2pActive: () => invoke<P2pShare | null>('p2p_active'),
  checkLauncherUpdate: () => invoke<LauncherUpdate | null>('check_launcher_update'),
  scanMods: (id: string) => invoke<ModReport[]>('scan_mods', { id }),
  scannerAllowMod: (id: string, file: string, allow: boolean) =>
    invoke<void>('scanner_allow_mod', { id, file, allow }),
  scannerQuarantine: (id: string) => invoke<string[]>('scanner_quarantine', { id }),
  scannerQuarantineRestore: (id: string, name: string) =>
    invoke<void>('scanner_quarantine_restore', { id, name }),
  scannerQuarantineDelete: (id: string, name: string) =>
    invoke<void>('scanner_quarantine_delete', { id, name }),
  aiScanNote: (id: string, file: string, lang: string) =>
    invoke<AiNote>('ai_scan_note', { id, file, lang }),
  syncMasterOptions: () => invoke<Record<string, string>>('sync_master_options'),
  findClassOwner: (id: string, cls: string) =>
    invoke<ClassOwner | null>('find_class_owner', { id, class: cls }),

  serversCatalog: (refresh: boolean) => invoke<CatalogServer[]>('servers_catalog', { refresh }),
  serverPing: (ip: string) => invoke<PingInfo>('server_ping', { ip }),
  serversDatAdd: (id: string, name: string, ip: string) =>
    invoke<void>('servers_dat_add', { id, name, ip }),
  pinnedServers: (id: string) => invoke<PinnedServer[]>('pinned_servers', { id }),
  pinServer: (id: string, name: string, ip: string, pin: boolean) =>
    invoke<void>('pin_server', { id, name, ip, pin }),
  gameMonitor: (id: string) => invoke<MonitorSample>('game_monitor', { id }),

  benchmarkStart: (id: string, world: string | null) =>
    invoke<void>('benchmark_start', { id, world }),
  benchmarkCancel: () => invoke<void>('benchmark_cancel'),
  benchmarkList: (id: string) => invoke<BenchRun[]>('benchmark_list', { id }),
};
