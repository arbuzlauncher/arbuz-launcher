// Типы данных и обёртки над Tauri-командами
import { invoke } from '@tauri-apps/api/core';
import type { CustomTheme } from './theme';

/** Корень данных лаунчера: папка `arbuz-launcher` рядом с лаунчером
 *  (или в профиле пользователя) — путь считает Rust, чтобы он совпадал. */
export const dataRoot = (): Promise<string> => invoke<string>('data_dir');

export type Loader = 'vanilla' | 'fabric' | 'forge' | 'neoforge' | 'quilt';
export type AccountKind = 'microsoft' | 'elyby' | 'littleskin' | 'offline' | 'plugin';

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
  /** «Ускоритель лаунчера»: локальный прокси с разбиением TLS ClientHello и HTTP-запросов. */
  launcherAccel: boolean;
  /** Ручной прокси для HTTP(S)-запросов лаунчера. */
  proxyEnabled: boolean;
  /** http | socks5 | socks5h. */
  proxyKind: string;
  proxyHost: string;
  proxyPort: number;
  proxyUser: string;
  /** Хранится в config.json; в webview приходит маской. */
  proxyPass: string;
  /** Закрытие окна прячет лаунчер в трей. */
  closeToTray: boolean;
  hideEmpty: boolean;
  modIsolationEnabled: boolean;
  modIsolationLevel: string;
  modIsolationClipboard: boolean;
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
  /** Провайдер ИИ: opencode | openai | openrouter | groq | deepseek | gemini | anthropic | custom. */
  aiProvider: string;
  /** API-ключ провайдера. В config.json хранится как есть, в webview приходит маской. */
  aiApiKey: string;
  /** Свой base URL для провайдера `custom`. */
  aiBaseUrl: string;
  /** Модель для чата и диагностики: пусто — дефолт провайдера. */
  aiModel: string;
  /** Экспериментально: Python-плагины (`.aplug`) и архивы `.arbz`. */
  pluginsEnabled: boolean;
  /** Настройки плагинов: id → произвольный JSON. */
  pluginSettings: Record<string, unknown>;
  /** Экспериментально: движок внешних тем. */
  themeEngine: boolean;
  /** Активная тема движка. */
  themePack: string;
  /** Динамический визуал Minecraft (панорама и текстуры из jar). */
  mcVisual: boolean;
  /** Автопроверка Fractureiser при запуске, установке модов и старте игры. */
  fractureiserGuard: boolean;
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
export type CatalogSource = 'modrinth' | 'curseforge' | 'ftb';

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

/** Готовое исправление из анализа лога. */
export interface LogFix {
  /** increase_ram | reset_java | reinstall_loader | open_instance | ... */
  kind: string;
  label: string;
  value: string | null;
}

/** Найденная в логе проблема. */
export interface LogFinding {
  id: string;
  severity: 'error' | 'warn' | 'info' | string;
  title: string;
  detail: string;
  fixes: LogFix[];
}

/** Запись журнала запусков профиля. */
export interface RunRecord {
  instanceId: string;
  started: number;
  ended: number;
  exitCode: number | null;
  log: string | null;
  crash: string | null;
}

/** Результат анализа свежего лога или краш-репорта. */
export interface LogAnalysis {
  found: boolean;
  file: string | null;
  findings: LogFinding[];
  journal: RunRecord[];
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

/** Сохранённая версия конфига. */
export interface ConfigVersion {
  stamp: number;
  size: number;
}

/** Результат проверки синтаксиса конфига. */
export interface ConfigCheck {
  ok: boolean;
  message: string;
}

/** Замечание проверки сборки. */
export interface PackIssue {
  severity: 'error' | 'warn' | 'info' | string;
  title: string;
  detail: string;
}

/** Готовое действие анализатора конфликтов. */
export interface ConflictAction {
  id: string;
  label: string;
  /** disable | enable | remove | open_catalog */
  kind: string;
  files: string[];
  modId: string | null;
  query: string | null;
}

/** Найденный конфликт модов. */
export interface ModConflict {
  id: string;
  /** duplicate | loader | missing_dep | disabled_dep | known */
  kind: string;
  severity: 'error' | 'warn' | 'info' | string;
  title: string;
  detail: string;
  actions: ConflictAction[];
}

/** Мод в сохранённом списке. */
export interface SavedMod {
  source: string;
  projectId: string;
  kind: string;
  name: string;
  version: string;
}

/** Сохранённый список модов профиля. */
export interface SavedModList {
  id: string;
  name: string;
  created: number;
  mcVersion: string;
  loader: string;
  mods: SavedMod[];
}

/** Результат сборки серверного пака. */
export interface ServerPackResult {
  path: string;
  files: number;
  bytes: number;
  loader: string;
  mcVersion: string;
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

/** План исправления зависимостей от ИИ/плагинов. */
export interface DependencyPlan {
  updates: UpdateInfo[];
  issues: PackIssue[];
  note: string;
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

/** Сервер из servers.dat профиля. */
export interface ServerEntry {
  name: string;
  ip: string;
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

/** Установленный Python-плагин (`.aplug`). */
export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  api: number;
  hooks: string[];
  permissions: string[];
  enabled: boolean;
  error: string | null;
  path: string;
  icon: string | null;
  homepage: string | null;
}

/** Панель, зарегистрированная плагином (например, игра 2048). */
export interface PluginPanel {
  id: string;
  title: string;
  icon?: string;
  description?: string;
  height?: number;
  plugin: string;
}

/** Пункт сайдбара, зарегистрированный плагином. */
export interface PluginSidebarItem {
  id: string;
  title: string;
  icon?: string;
  panel?: string;
  order?: number;
  badge?: string;
  description?: string;
  plugin: string;
}

/** Строка лога плагина. */
export interface PluginLogEntry {
  plugin: string;
  level: string;
  message: string;
  ts: number;
}

/** Пункт, который плагин добавил в универсальный слот интерфейса. */
export interface PluginSlotItem {
  id?: string;
  label: string;
  icon?: string;
  command?: string;
  args?: unknown;
  panel?: string;
  plugin?: string;
}

/** Результат хука слота: пункты, HTML и патч элемента. */
export interface PluginSlotResult {
  items: PluginSlotItem[];
  html: string;
  patch: Record<string, unknown>;
}

/** Патч элемента интерфейса из хука `ui.element`. */
export interface PluginElementPatch {
  hidden?: boolean;
  text?: string;
  html?: string;
  className?: string;
  style?: Record<string, string>;
  before?: string;
  after?: string;
  [key: string]: unknown;
}

/** Предупреждение о плагине до установки. */
export interface PluginPreviewWarning {
  level: string;
  code: string;
  message: string;
}

/** Файл внутри плагина для предпросмотра. */
export interface PluginPreviewFile {
  name: string;
  size: number;
  binary: boolean;
  code: string | null;
}

/** Полное превью `.aplug`/`.arbz` до установки. */
export interface PluginPreview {
  path: string;
  fileName: string;
  size: number;
  sha256: string;
  kind: string;
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  api: number;
  permissions: string[];
  hooks: string[];
  files: PluginPreviewFile[];
  warnings: PluginPreviewWarning[];
  network: boolean;
  obfuscated: boolean;
  dangerous: boolean;
  risk: string;
}

/** Содержимое панели плагина: HTML/CSS/JS для изолированного iframe. */
export interface PluginPanelContent {
  html?: string;
  css?: string;
  js?: string;
  height?: number;
  title?: string;
  error?: string;
  [key: string]: unknown;
}

/** Тема движка тем. */
export interface ThemePack {
  id: string;
  name: string;
  author: string;
  description: string;
  dark: boolean;
  colors: Record<string, string>;
  radius: number;
  font: string;
  css: string;
  layout: string;
  soft: boolean;
  plugin: string | null;
}

export interface ThemeLoad {
  theme: ThemePack;
  css: string;
}

/** Визуал Minecraft из jar версии. */
export interface McVisual {
  version: string;
  title: string;
  panorama: string[];
  faces?: number;
  button: Record<string, string>;
  accent: string;
  css?: string;
  theme?: string;
  layout?: string;
  error?: string;
  [key: string]: unknown;
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
  aiDependencyPlan: (id: string) => invoke<DependencyPlan>('ai_dependency_plan', { id }),
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
  aiTest: () => invoke<string>('ai_test'),
  openCrashReports: (id: string) => invoke<void>('open_crash_reports', { id }),

  analyzeLogs: (id: string) => invoke<LogAnalysis>('analyze_logs', { id }),
  logJournal: (id: string) => invoke<RunRecord[]>('log_journal', { id }),
  logSupportReport: (id: string) => invoke<string>('log_support_report', { id }),
  logSaveReport: (id: string, dest: string) => invoke<void>('log_save_report', { id, dest }),
  logRules: () => invoke<unknown[]>('log_rules'),
  logUpdateRules: (url: string) => invoke<number>('log_update_rules', { url }),
  logApplyFix: (id: string, kind: string, value?: string | null) =>
    invoke<void>('log_apply_fix', { id, kind, value: value ?? null }),

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
  packConfigHistory: (id: string, path: string) =>
    invoke<ConfigVersion[]>('pack_config_history', { id, path }),
  packConfigVersion: (id: string, path: string, stamp: number) =>
    invoke<string>('pack_config_version', { id, path, stamp }),
  packConfigDiff: (id: string, path: string, stamp: number) =>
    invoke<string>('pack_config_diff', { id, path, stamp }),
  validatePackConfig: (path: string, content: string) =>
    invoke<ConfigCheck>('validate_pack_config', { path, content }),
  validatePack: (id: string) => invoke<PackIssue[]>('validate_pack', { id }),
  analyzeConflicts: (id: string) => invoke<ModConflict[]>('analyze_conflicts', { id }),
  modlistsList: () => invoke<SavedModList[]>('modlists_list'),
  modlistSave: (id: string, name: string) =>
    invoke<SavedModList>('modlist_save', { id, name }),
  modlistDelete: (id: string) => invoke<void>('modlist_delete', { id }),
  modlistInstall: (listId: string, instanceId: string) =>
    invoke<number>('modlist_install', { listId, instanceId }),
  buildServerPack: (args: {
    instanceId: string | null;
    kind: string;
    mcVersion: string;
    loaderVersion: string | null;
    dest: string;
    ramMb: number;
  }) => invoke<ServerPackResult>('build_server_pack', args),
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
  serversDatList: (id: string) => invoke<ServerEntry[]>('servers_dat_list', { id }),
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

  // Плагины
  pluginsList: () => invoke<PluginInfo[]>('plugins_list'),
  pluginInspect: (path: string) => invoke<PluginPreview>('plugin_inspect', { path }),
  pluginInstall: (path: string) => invoke<PluginInfo[]>('plugin_install', { path }),
  pluginUninstall: (id: string) => invoke<void>('plugin_uninstall', { id }),
  pluginSetEnabled: (id: string, enabled: boolean) =>
    invoke<void>('plugin_set_enabled', { id, enabled }),
  pluginReload: () => invoke<void>('plugin_reload'),
  pluginOpenFolder: () => invoke<void>('plugin_open_folder'),
  pluginOpenLibs: () => invoke<void>('plugin_open_libs'),
  pluginPanels: () => invoke<PluginPanel[]>('plugin_panels'),
  pluginSidebar: () => invoke<PluginSidebarItem[]>('plugin_sidebar'),
  pluginLogs: () => invoke<PluginLogEntry[]>('plugin_logs'),
  pluginLogsClear: () => invoke<void>('plugin_logs_clear'),
  pluginUiSlot: (slot: string, props?: unknown) =>
    invoke<PluginSlotResult>('plugin_ui_slot', { slot, props: props ?? null }),
  pluginUiElement: (id: string, props?: unknown) =>
    invoke<PluginElementPatch>('plugin_ui_element', { id, props: props ?? null }),
  pluginStorageGet: (plugin: string, key: string) =>
    invoke<unknown>('plugin_storage_get', { plugin, key }),
  pluginStorageSet: (plugin: string, key: string, value: unknown) =>
    invoke<void>('plugin_storage_set', { plugin, key, value }),
  pluginStorageAll: (plugin: string) =>
    invoke<Record<string, unknown>>('plugin_storage_all', { plugin }),
  pluginConfigGet: (plugin: string, key: string) =>
    invoke<unknown>('plugin_config_get', { plugin, key }),
  pluginConfigSet: (plugin: string, key: string, value: unknown) =>
    invoke<void>('plugin_config_set', { plugin, key, value }),
  pluginPanelEvent: (plugin: string, event: string, data?: unknown) =>
    invoke<unknown>('plugin_panel_event', { plugin, event, data: data ?? null }),
  pluginSources: () => invoke<Record<string, unknown>[]>('plugin_sources'),
  pluginCommands: () => invoke<Record<string, unknown>[]>('plugin_commands'),
  pluginPanelRender: (plugin: string, panel: string, args?: unknown) =>
    invoke<PluginPanelContent>('plugin_panel_render', { plugin, panel, args: args ?? null }),
  pluginRunCommand: (plugin: string, command: string, args?: unknown) =>
    invoke<unknown>('plugin_run_command', { plugin, command, args: args ?? null }),
  pluginCatalogSearch: (source: string, args: unknown) =>
    invoke<Record<string, unknown>[]>('plugin_catalog_search', { source, args }),
  pluginAuthProviders: () => invoke<Record<string, unknown>[]>('plugin_auth_providers'),
  pluginAuthLogin: (provider: string, credentials: Record<string, string>) =>
    invoke<Account>('plugin_auth_login', { provider, credentials }),
  pluginVisual: (instanceId: string) =>
    invoke<McVisual>('plugin_visual', { instanceId }),

  // Движок тем
  themesList: () => invoke<ThemePack[]>('themes_list'),
  themeLoad: (id: string) => invoke<ThemeLoad>('theme_load', { id }),
  themeInstall: (path: string) => invoke<string>('theme_install', { path }),
  themeRemove: (id: string) => invoke<void>('theme_remove', { id }),
  themeOpenFolder: () => invoke<void>('theme_open_folder'),
  themeExport: (id: string) => invoke<{ theme: ThemePack; css: string }>('theme_export', { id }),

  // Самообновление
  launcherUpdateDownload: () => invoke<string>('launcher_update_download'),
  launcherUpdateInstall: (path: string) => invoke<void>('launcher_update_install', { path }),
};
