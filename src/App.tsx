import { useEffect, useLayoutEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { AccountMenu } from './components/AccountMenu';
import { Accounts } from './components/Accounts';
import { AddAccountModal } from './components/AddAccountModal';
import { AppSettings } from './components/AppSettings';
import { BottomBar } from './components/BottomBar';
import { Catalog } from './components/Catalog';
import { ConsoleModal } from './components/ConsoleModal';
import { CoopTab } from './components/CoopTab';
import { ConfirmModal } from './components/ConfirmModal';
import { CrashModal } from './components/CrashModal';
import { Home } from './components/Home';
import { Installations } from './components/Installations';
import { LayoutEdit } from './components/LayoutEdit';
import { LauncherUpdateModal } from './components/LauncherUpdateModal';
import { Onboarding } from './components/Onboarding';
import { ProfileView } from './components/ProfileView';
import { ServersView } from './components/ServersView';
import { Sidebar } from './components/Sidebar';
import { Titlebar } from './components/Titlebar';
import { Toast } from './components/Toast';
import { api, dataRoot } from './lib/api';
import { onGameExit, onGameLog, onGameStart, onProgress } from './lib/events';
import { applyTheme, normCustom, normStyle, softOf } from './lib/theme';
import { animActive, applyAnim, mergeAnim, useReducedMotion } from './lib/anim';
import { normLang } from './lib/i18n';
import { applyDirection } from './lib/rtl';
import { pushLog, flushConfig, loadContentCounts, loadSkins, useStore, type StoreState } from './store';

export default function App() {
  const view = useStore((s) => s.view);
  const onboarded = useStore((s) => s.config.onboarded);
  const lang = useStore((s) => s.config.lang);
  const style = useStore((s) => s.config.style);
  const dark = useStore((s) => s.config.dark);
  const accent = useStore((s) => s.config.accent);
  const custom = useStore((s) => s.config.custom);
  const anim = useStore((s) => s.config.anim);
  const move = useStore((s) => s.move);
  const reduce = useReducedMotion();
  const checkAppUpd = useStore((s) => s.config.checkLauncherUpdates);

  // Язык меняет не только строки, но и направление записи (RTL для ar/fa/he).
  useEffect(() => {
    applyDirection(lang);
  }, [lang]);

  // Ускоритель оставляем только в русском и английском интерфейсе.
  useEffect(() => {
    if (lang === 'ru' || lang === 'en') return;
    const st = useStore.getState();
    if (st.config.launcherAccel) {
      void st.setConfig({ ...st.config, launcherAccel: false });
    }
  }, [lang]);

  // Обновления лаунчера: один раз за запуск, если включено в настройках.
  useEffect(() => {
    if (!onboarded || !checkAppUpd) return;
    let alive = true;
    const timer = window.setTimeout(() => {
      api
        .checkLauncherUpdate()
        .then((u) => {
          if (alive && u) useStore.getState().set({ appUpdate: u });
        })
        .catch(() => {});
    }, 2500);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [onboarded, checkAppUpd]);

  // До первого кадра — только get_config и list_instances. Ошибка одного
  // запроса не должна мешать другому: конфиг грузится независимо от списка.
  useEffect(() => {
    let alive = true;
    void api
      .getConfig()
      .then((cfg) => {
        if (!alive) return;
        useStore.getState().set({ config: { ...cfg, style: cfg.experimentalThemes ? cfg.style : 'flat', lang: cfg.onboarded ? normLang(cfg.lang) : 'en', anim: mergeAnim(cfg.anim), custom: normCustom(cfg.custom) } });
      })
      .catch((e) => {
        if (alive) useStore.getState().toastMsg(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    void api
      .listInstances()
      .then((list) => {
        if (!alive) return;
        useStore.getState().set({ instances: list, sel: list[0]?.id ?? null });
      })
      .catch((e) => {
        if (alive) useStore.getState().toastMsg(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);

  // Отложенная запись настроек не должна теряться при закрытии окна.
  useEffect(() => {
    const flush = () => flushConfig();
    const onHide = () => {
      if (document.visibilityState === 'hidden') flush();
    };
    window.addEventListener('pagehide', flush);
    window.addEventListener('beforeunload', flush);
    document.addEventListener('visibilitychange', onHide);
    return () => {
      window.removeEventListener('pagehide', flush);
      window.removeEventListener('beforeunload', flush);
      document.removeEventListener('visibilitychange', onHide);
      flush();
    };
  }, []);

  // Лениво, после первого кадра: RAM, корень данных, аккаунты и список версий.
  useEffect(() => {
    let alive = true;
    (async () => {
      const [ram, root, accs, vers, jvs] = await Promise.allSettled([
        api.getSystemRamMb(),
        dataRoot(),
        api.listAccounts(),
        api.listMcVersions(),
        api.detectJavas(),
      ]);
      if (!alive) return;
      const st = useStore.getState();
      if (ram.status === 'fulfilled') st.set({ systemRamMb: ram.value });
      else st.toastMsg(String(ram.reason));
      if (root.status === 'fulfilled') st.set({ dataRoot: root.value });
      else st.toastMsg(String(root.reason));
      if (accs.status === 'fulfilled') {
        st.set({ accounts: accs.value });
      } else st.toastMsg(String(accs.reason));
      if (vers.status === 'fulfilled') st.set({ versions: vers.value });
      else st.toastMsg(String(vers.reason));
      if (jvs.status === 'fulfilled') st.set({ javas: jvs.value });
      else st.toastMsg(String(jvs.reason));
    })();
    return () => {
      alive = false;
    };
  }, []);

  // До отрисовки: тема и переменные анимаций должны быть на месте, иначе
  // первый кадр мигнёт дефолтными значениями. Разделено, чтобы перетаскивание
  // точек кривой не пересчитывало палитру.
  useLayoutEffect(() => {
    const el = document.getElementById('win');
    if (el) applyTheme(el, normStyle(style), dark, accent, custom);
  }, [style, dark, accent, custom]);

  useLayoutEffect(() => {
    const el = document.getElementById('win');
    if (el) applyAnim(el, mergeAnim(anim), reduce);
  }, [anim, reduce]);

  const animOn = animActive(mergeAnim(anim), reduce);

  // Новости — лениво после первого кадра, сеть может быть недоступна.
  useEffect(() => {
    let alive = true;
    void api
      .getNews()
      .then((news) => {
        if (alive) useStore.getState().set({ news });
      })
      .catch((e) => {
        if (alive) useStore.getState().toastMsg(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);

  // Для «Скрывать пустые профили» нужно знать число файлов контента в каждом профиле.
  const hideEmpty = useStore((s) => s.config.hideEmpty);
  const instances = useStore((s) => s.instances);
  useEffect(() => {
    if (!hideEmpty) return;
    void loadContentCounts().catch(() => {});
  }, [hideEmpty, instances]);

  // Скины для аватаров аккаунтов — тоже лениво, после первого кадра.
  const accounts = useStore((s) => s.accounts);
  useEffect(() => {
    if (accounts.length === 0) return;
    let alive = true;
    void loadSkins(accounts).catch(() => {
      if (!alive) return;
    });
    return () => {
      alive = false;
    };
  }, [accounts]);

  useEffect(() => {
    const un: Array<() => void> = [];
    void onProgress((p) => {
      const st = useStore.getState();
      // Установка сборки идёт под общим id "pack" (ТЗ §11) — показываем плашкой.
      if (p.id === 'pack') {
        st.set({ packPct: p.pct });
        return;
      }
      st.set({ running: { ...st.running, [p.id]: p.pct } });
    }).then((u) => un.push(u));
    void onGameLog((p) => {
      pushLog(p.id, p.line);
    }).then((u) => un.push(u));
    // Процесс игры запущен: кнопка становится «Остановить».
    void onGameStart((p) => {
      const st = useStore.getState();
      if (st.running[p.id] === undefined || st.live[p.id]) return;
      const patch: Partial<StoreState> = {
        live: { ...st.live, [p.id]: true },
        running: { ...st.running, [p.id]: 100 },
      };
      if (st.config.showConsole) patch.console = p.id;
      st.set(patch);
      if (st.config.closeOnLaunch) {
        void getCurrentWindow()
          .hide()
          .catch((e) => useStore.getState().toastMsg(String(e)));
      }
    }).then((u) => un.push(u));
    void onGameExit((p) => {
      const st = useStore.getState();
      const running = { ...st.running };
      delete running[p.id];
      const live = { ...st.live };
      delete live[p.id];
      const patch: Partial<StoreState> = { running, live };
      if (st.console === p.id) patch.console = null;
      if (p.code !== null && p.code !== 0) patch.crash = { id: p.id, code: p.code };
      st.set(patch);
      if (st.config.closeOnLaunch) {
        void getCurrentWindow()
          .show()
          .then(() => getCurrentWindow().setFocus())
          .catch((e) => useStore.getState().toastMsg(String(e)));
      }
      api
        .listInstances()
        .then((list) => useStore.getState().set({ instances: list }))
        .catch((e) => useStore.getState().toastMsg(String(e)));
    }).then((u) => un.push(u));
    return () => un.forEach((u) => u());
  }, []);

  return (
    <div id="win" data-v={softOf(normStyle(style), custom) ? 'b' : 'a'} data-anim={animOn ? 'on' : 'off'} data-move={move ? 'on' : 'off'}>
      <Titlebar />
      {onboarded ? (
        <div id="body">
          <Sidebar />
          <div id="mn" key={animOn ? 'anim-on' : 'anim-off'}>
            <div className="screen-scroll">
              {view === 'home' ? (
                <Home />
              ) : view === 'installs' ? (
                <Installations />
              ) : view === 'coop' ? (
                <CoopTab />
              ) : view === 'servers' ? (
                <ServersView />
              ) : view === 'accounts' ? (
                <Accounts />
              ) : view === 'p' ? (
                <ProfileView />
              ) : view === 'browse' ? (
                <Catalog />
              ) : (
                <AppSettings />
              )}
            </div>
            <BottomBar />
          </div>
        </div>
      ) : (
        <Onboarding />
      )}
      <Toast />
      <AddAccountModal />
      <ConfirmModal />
      <CrashModal />
      <LauncherUpdateModal />
      {onboarded && <ConsoleModal />}
      {onboarded && <AccountMenu />}
      {onboarded && <LayoutEdit />}
    </div>
  );
}
