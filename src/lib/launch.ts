// Запуск и остановка профилей игры
import { api } from "./api";
import { t } from "./i18n";
import { askConfirm, useStore } from "../store";

/** Идёт ли процесс игры для профиля (запускается или уже запущен). */
export function isRunning(
  s: { running: Record<string, number>; live: Record<string, boolean> },
  id: string,
): boolean {
  return s.live[id] === true || s.running[id] !== undefined;
}

/** Текст этапа запуска по проценту готовности. */
export function stat(p: number): string {
  return p < 30
    ? t("Проверка файлов игры…")
    : p < 65
      ? t("Загрузка библиотек…")
      : p < 90
        ? t("Скачивание ассетов…")
        : t("Запуск Java…");
}

/** Запускает профиль или останавливает его, если он уже работает.
 * `scanOverride` — «запустить всё равно» после предупреждения сканера. */
export async function playInstance(
  id: string,
  world: string | null = null,
  server: string | null = null,
): Promise<void> {
  const st = useStore.getState();
  if (isRunning(st, id)) {
    try {
      await api.stopInstance(id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
    return;
  }
  useStore.getState().set({ running: { ...st.running, [id]: 0 } });
  try {
    await api.launchInstance(id, world, server, false);
  } catch (e) {
    const s2 = useStore.getState();
    const running = { ...s2.running };
    delete running[id];
    const live = { ...s2.live };
    delete live[id];
    s2.set({ running, live });
    await onLaunchError(id, world, String(e));
  }
}

/** Ошибка запуска: блокировка сканера в обычном режиме предлагает
 * «запустить всё равно» с подтверждением; параноидальный режим — только
 * через явное исключение мода во вкладке «Безопасность». */
async function onLaunchError(id: string, world: string | null, msg: string): Promise<void> {
  const st = useStore.getState();
  if (!msg.startsWith("Запуск остановлен сканером модов")) {
    st.toastMsg(msg);
    return;
  }
  const inst = st.instances.find((i) => i.id === id);
  const mode = inst?.scannerMode || st.config.scannerMode;
  if (mode === "paranoid") {
    st.toastMsg(msg);
    return;
  }
  const ok = await askConfirm({
    title: t("Сканер нашёл подозрительные признаки"),
    text: msg + "\n\n" + t("Список причин — во вкладке «Безопасность» профиля. Запустить всё равно?"),
    yes: t("Запустить всё равно"),
    no: t("Отмена"),
    danger: true,
  });
  if (!ok) return;
  const s2 = useStore.getState();
  s2.set({ running: { ...s2.running, [id]: 0 } });
  try {
    await api.launchInstance(id, world, null, true);
  } catch (e) {
    const s3 = useStore.getState();
    const running = { ...s3.running };
    delete running[id];
    s3.set({ running });
    s3.toastMsg(String(e));
  }
}
