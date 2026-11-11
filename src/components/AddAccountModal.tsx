// Модалка добавления аккаунта (.mod из макета, функция modal())
import { useCallback, useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { api, type DeviceCode } from '../lib/api';
import { onMsStep } from '../lib/events';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

type Kind = 'ms' | 'ely' | 'off' | 'lsk';
type ModalKind = Kind | 'pick' | 'plug';

interface AuthProvider {
  id: string;
  name: string;
  description?: string;
  plugin: string;
  hint?: string;
  fields?: { key: string; label?: string; type?: string; placeholder?: string }[];
}

const KINDS: [Kind, string][] = [
  ['ms', 'Microsoft'],
  ['ely', 'Ely.by'],
  ['lsk', 'LittleSkin'],
  ['off', 'Offline-аккаунт'],
];

/** Короткое пояснение к каждому способу входа. */
const HINTS: Record<Kind, string> = {
  ms: 'Вход по коду в браузере. Нужен аккаунт Microsoft с Minecraft.',
  ely: 'Аккаунт Ely.by — логин и пароль.',
  lsk: 'Аккаунт LittleSkin — логин и пароль.',
  off: 'Без аккаунта: введите ник и играйте. Серверы доступны, но скина не будет.',
};

const KIND_ICON: Record<Kind, ReactNode> = {
  ms: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="3" y="3" width="8" height="8" />
      <rect x="13" y="3" width="8" height="8" />
      <rect x="3" y="13" width="8" height="8" />
      <rect x="13" y="13" width="8" height="8" />
    </svg>
  ),
  ely: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3 4 6v6c0 4.5 3.4 7.6 8 9 4.6-1.4 8-4.5 8-9V6l-8-3Z" />
      <path d="m9 12 2 2 4-4" />
    </svg>
  ),
  lsk: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 7h7v7H4zM13 7h7v7h-7zM4 16h7v4H4zM13 16h7v4h-7z" />
    </svg>
  ),
  off: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="8" r="4" />
      <path d="M4 21c0-4.4 3.6-7 8-7s8 2.6 8 7" />
    </svg>
  ),
};

/** Ссылка, на которой Microsoft подставляет код сам. */
const linkOf = (d: DeviceCode): string =>
  d.verification_url || `${d.verification_uri}?otc=${encodeURIComponent(d.user_code)}`;

const shortLink = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\?.*$/, '');

export function AddAccountModal() {
  const modal = useStore((s) => s.modal);
  const t = useT();
  const [name, setName] = useState('');
  const [login, setLogin] = useState('');
  const [password, setPassword] = useState('');
  const [dc, setDc] = useState<DeviceCode | null>(null);
  const [err, setErr] = useState('');
  const [step, setStep] = useState('');
  const [busy, setBusy] = useState(false);
  const [providers, setProviders] = useState<AuthProvider[]>([]);
  const [cred, setCred] = useState<Record<string, string>>({});

  // Автостарт делаем один раз за открытие модалки: если вход сорвался,
  // следующий код пользователь открывает сам, без лавины окон браузера.
  const autoRef = useRef(false);
  // Пока модалка на вкладке Microsoft — ответы бэкенда ещё имеют смысл.
  const liveRef = useRef(false);

  const kind: ModalKind | null = modal?.p ?? null;
  const providerId = modal?.p === 'plug' ? modal.provider : '';
  const provider = providers.find((p) => p.id === providerId);

  useEffect(() => {
    void api
      .pluginAuthProviders()
      .then((list) => setProviders(list as unknown as AuthProvider[]))
      .catch(() => {});
  }, []);

  /** Общий финал успешного входа: список аккаунтов, активный, закрытие. */
  const done = useCallback(async (id: string) => {
    const list = await api.listAccounts();
    const st = useStore.getState();
    st.set({
      accounts: list,
      config: { ...st.config, activeAccount: id },
      modal: null,
      menu: false,
    });
    st.toastMsg(t('Аккаунт добавлен'));
    setName('');
    setLogin('');
    setPassword('');
  }, []);

  // Этапы входа приходят с бэкенда, чтобы пользователь видел, где именно
  // процесс остановился, а не просто «ничего не произошло».
  useEffect(() => {
    let un: (() => void) | null = null;
    let dead = false;
    void onMsStep((p) => {
      if (!dead) setStep(t(p.text));
    }).then((f) => {
      if (dead) f();
      else un = f;
    });
    return () => {
      dead = true;
      un?.();
    };
  }, []);

  // Код устройства запрашиваем при входе на вкладку Microsoft, а при уходе
  // с неё или закрытии диалога — отменяем ожидание (ТЗ §12-a.8).
  useEffect(() => {
    if (kind !== 'ms') return;
    autoRef.current = false;
    liveRef.current = true;
    setDc(null);
    setErr('');
    setStep('');
    setBusy(false);
    api
      .msBegin()
      .then((d) => {
        if (liveRef.current) setDc(d);
      })
      .catch((e) => {
        if (liveRef.current) setErr(String(e));
      });
    return () => {
      liveRef.current = false;
      void api.msCancel().catch(() => {});
    };
  }, [kind]);

  // Как только код получен — сразу открываем браузер и начинаем ждать.
  useEffect(() => {
    if (kind !== 'ms' || !dc || autoRef.current) return;
    autoRef.current = true;
    setBusy(true);
    setErr('');
    setStep(t('Открываем страницу входа…'));
    const poll = api.msPoll(dc.device_code, dc.interval);
    openUrl(linkOf(dc))
      .then(() => {
        if (liveRef.current) setStep(t('Ожидание подтверждения в браузере'));
      })
      .catch(() => {
        if (liveRef.current) setStep(t('Откройте ссылку ниже в браузере и подтвердите вход'));
      });
    poll
      .then(async (acc) => {
        setStep('');
        await done(acc.id);
      })
      .catch((e) => {
        if (!liveRef.current) return;
        const msg = String(e);
        setStep('');
        setBusy(false);
        setErr(msg);
        useStore.getState().toastMsg(msg);
        api
          .msBegin()
          .then((n) => {
            if (liveRef.current) setDc(n);
          })
          .catch(() => {});
      });
  }, [dc, kind, done]);

  if (kind === null) return null;

  const close = () => {
    void api.msCancel().catch(() => {});
    useStore.getState().set({ modal: null });
    setName('');
    setLogin('');
    setPassword('');
    setErr('');
    setStep('');
    setDc(null);
  };

  const closeOnBackdrop = (e: MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) close();
  };

  const addOffline = async () => {
    const nick = name.trim();
    if (!nick) {
      useStore.getState().toastMsg(t('Введите никнейм'));
      return;
    }
    try {
      const acc = await api.offlineAdd(nick);
      // Ник однозначно задаёт UUID, поэтому берём список с бэкэнда:
      // повторное добавление того же ника не создаёт дубликат в интерфейсе.
      await done(acc.id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
      try {
        const list = await api.listAccounts();
        useStore.getState().set({ accounts: list });
      } catch (inner) {
        useStore.getState().toastMsg(String(inner));
      }
    }
  };

  const addPass = async (k: 'ely' | 'lsk') => {
    if (!login.trim() || !password) {
      useStore.getState().toastMsg(t('Введите логин и пароль'));
      return;
    }
    setBusy(true);
    try {
      const acc = k === 'ely' ? await api.elybyLogin(login.trim(), password) : await api.littleLogin(login.trim(), password);
      await done(acc.id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const addPlugin = async () => {
    if (!provider) return;
    setBusy(true);
    try {
      const acc = await api.pluginAuthLogin(provider.id, cred);
      await done(acc.id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const msReopen = async () => {
    if (!dc || busy) return;
    await openUrl(linkOf(dc)).catch((e) => setErr(String(e)));
  };

  let form = null;
  if (kind === 'pick') {
    form = (
      <>
        <h2>{t('Добавить аккаунт')}</h2>
        <p className="hint" style={{ marginBottom: 16 }}>
          {t('Выберите, как войти в игру')}
        </p>
        <div className="acc-pick">
          {KINDS.map(([k, label]) => (
            <button
              key={k}
              type="button"
              className="acc-card"
              onClick={() => useStore.getState().set({ modal: { p: k } })}
            >
              <span className={'acc-ic ' + k}>{KIND_ICON[k]}</span>
              <span className="acc-tx">
                <b>{t(label)}</b>
                <span className="hint">{t(HINTS[k])}</span>
              </span>
              <span className="acc-go">→</span>
            </button>
          ))}
          {providers.map((p) => (
            <button
              key={p.id}
              type="button"
              className="acc-card"
              onClick={() => {
                setCred({});
                useStore.getState().set({ modal: { p: 'plug', provider: p.id } });
              }}
            >
              <span className="acc-ic off">🔌</span>
              <span className="acc-tx">
                <b>{p.name}</b>
                <span className="hint">{p.description || t('Способ входа от плагина')}</span>
              </span>
              <span className="acc-go">→</span>
            </button>
          ))}
        </div>
      </>
    );
  } else if (kind === 'ms') {
    form = (
      <>
        <h2>{t('Вход через Microsoft')}</h2>
        <p className="hint" style={{ marginBottom: 14 }}>
          {t('Страница входа откроется в браузере, код подставится сам. Лаунчер получит токены Xbox и Minecraft.')}
        </p>
        {!dc ? (
          <div className="panel" style={{ padding: 14, background: 'var(--s2)' }}>
            <b style={{ fontWeight: 500 }}>{t('Вход недоступен')}</b>
            <p className="hint" style={{ marginTop: 6 }}>
              {err || t('Получаем код устройства…')}
            </p>
          </div>
        ) : (
          <>
            <div
              className="panel"
              style={{ padding: 16, textAlign: 'center', background: 'var(--s2)', cursor: 'pointer' }}
              onClick={() => void msReopen()}
              title={t('Открыть страницу входа ещё раз')}
            >
              <div className="hint">{t('Код устройства')}</div>
              <b style={{ fontSize: 28, letterSpacing: '.18em', fontWeight: 600 }}>
                {dc.user_code}
              </b>
              <div className="hint">{shortLink(linkOf(dc))}</div>
            </div>
            <p className="hint" style={{ marginTop: 10, minHeight: 16 }}>
              {step || t('Код уже подставлен — подтвердите вход в браузере')}
            </p>
            {err && (
              <p className="hint" style={{ marginTop: 10, color: 'var(--err, #d9503f)' }}>
                {err}
              </p>
            )}
            <div className="row g8" style={{ marginTop: 16 }}>
              <button className="btn pr" disabled={busy} onClick={() => void msReopen()}>
                {t('Открыть браузер')}
              </button>
            </div>
          </>
        )}
      </>
    );
  } else if (kind === 'ely' || kind === 'lsk') {
    form = (
      <>
        <h2>{t(kind === 'ely' ? 'Вход через Ely.by' : 'Вход через LittleSkin')}</h2>
        <label className="l">{t('Логин или e-mail')}</label>
        <input
          type="text"
          value={login}
          onChange={(e) => setLogin(e.target.value)}
          placeholder="Steve"
        />
        <label className="l">{t('Пароль')}</label>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="••••••••"
        />
        <div className="sw">
          <div>
            <b style={{ fontWeight: 500 }}>authlib-injector</b>
            <div className="hint">{t('Подключается автоматически при запуске')}</div>
          </div>
          <div className="tg on" />
        </div>
        <button className="btn pr" disabled={busy} onClick={() => void addPass(kind)}>
          {t('Войти')}
        </button>
      </>
    );
  } else if (kind === 'plug') {
    const fields = provider?.fields?.length
      ? provider.fields
      : [
          { key: 'login', label: t('Логин или e-mail'), type: 'text', placeholder: 'Steve' },
          { key: 'password', label: t('Пароль'), type: 'password', placeholder: '••••••••' },
        ];
    form = (
      <>
        <h2>{provider?.name || t('Вход через плагин')}</h2>
        <p className="hint" style={{ marginBottom: 14 }}>
          {provider?.hint || provider?.description || t('Этот способ входа добавлен плагином.')}
        </p>
        {fields.map((f) => (
          <div key={f.key}>
            <label className="l">{f.label || f.key}</label>
            <input
              type={f.type || 'text'}
              value={cred[f.key] ?? ''}
              placeholder={f.placeholder}
              onChange={(e) => setCred((c) => ({ ...c, [f.key]: e.target.value }))}
            />
          </div>
        ))}
        <button className="btn pr" disabled={busy || !provider} onClick={() => void addPlugin()}>
          {busy ? t('Входим…') : t('Войти')}
        </button>
        {provider?.plugin && (
          <p className="hint" style={{ marginTop: 10 }}>
            {t('Провайдер плагина: {p}', { p: provider.plugin })}
          </p>
        )}
      </>
    );
  } else {
    form = (
      <>
        <h2>{t('Offline-аккаунт')}</h2>
        <label className="l">{t('Никнейм')}</label>
        <input
          type="text"
          maxLength={16}
          placeholder="Steve"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <p className="hint" style={{ margin: '8px 0 16px' }}>
          {t('От 3 до 16 символов. Аккаунт не нужен: можно играть на серверах, но скина не будет.')}
        </p>
        <button className="btn pr" onClick={() => void addOffline()}>
          {t('Добавить')}
        </button>
      </>
    );
  }

  return (
    <div className="ov" onClick={closeOnBackdrop}>
      <div className="mod acc-mod">
        <div className="r">
          {kind !== 'pick' && (
            <button
              type="button"
              className="acc-back"
              onClick={() => useStore.getState().set({ modal: { p: 'pick' } })}
            >
              ← {t('Назад')}
            </button>
          )}
          {form}
        </div>
      </div>
    </div>
  );
}