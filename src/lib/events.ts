// Подписки на события бэкенда
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface ProgressPayload {
  id: string;
  pct: number;
}

export interface GameLogPayload {
  id: string;
  line: string;
}

export interface GameExitPayload {
  id: string;
  code: number | null;
}

/** Игровой процесс стартовал. */
export interface GameStartPayload {
  id: string;
}

/** Текущий этап входа через Microsoft. */
export interface MsStepPayload {
  text: string;
}

export const onProgress = (h: (p: ProgressPayload) => void): Promise<UnlistenFn> =>
  listen<ProgressPayload>('progress', (e) => h(e.payload));

export const onGameLog = (h: (p: GameLogPayload) => void): Promise<UnlistenFn> =>
  listen<GameLogPayload>('game-log', (e) => h(e.payload));

export const onGameStart = (h: (p: GameStartPayload) => void): Promise<UnlistenFn> =>
  listen<GameStartPayload>('game-start', (e) => h(e.payload));

export const onGameExit = (h: (p: GameExitPayload) => void): Promise<UnlistenFn> =>
  listen<GameExitPayload>('game-exit', (e) => h(e.payload));

export const onMsStep = (h: (p: MsStepPayload) => void): Promise<UnlistenFn> =>
  listen<MsStepPayload>('ms-step', (e) => h(e.payload));

/** Бенчмарк завершился: итоги прогона. */
export const onBenchDone = (h: (p: unknown) => void): Promise<UnlistenFn> =>
  listen<unknown>('bench-done', (e) => h(e.payload));

/** Fractureiser нашёл заражённые файлы при фоновой проверке. */
export interface SecurityAlertPayload {
  source: string;
  hits: Array<{
    file: string;
    sha256: string;
    reason: string;
    instance: string | null;
    quarantined: boolean;
  }>;
}

export const onSecurityAlert = (h: (p: SecurityAlertPayload) => void): Promise<UnlistenFn> =>
  listen<SecurityAlertPayload>('security-alert', (e) => h(e.payload));

/** Событие от плагина: toast, регистрация темы/источника, visual.changed и т.п. */
export interface PluginEventPayload {
  event: string;
  payload: Record<string, unknown>;
}

export const onPluginEvent = (h: (p: PluginEventPayload) => void): Promise<UnlistenFn> =>
  listen<PluginEventPayload>('plugin-event', (e) => h(e.payload));
