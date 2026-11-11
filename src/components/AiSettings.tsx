import { useEffect, useState } from 'react';
import { api, type Config } from '../lib/api';
import { useT } from '../lib/i18n';
import { flushConfig, useStore } from '../store';
import { Toggle } from './Controls';

/** Провайдеры ИИ. `opencode` — встроенный бесплатный, остальные — по API-ключу. */
const AI_PROVIDERS: {
  id: string;
  name: string;
  hint: string;
  model: string;
  keyHint: string;
}[] = [
  {
    id: 'opencode',
    name: 'OpenCode Zen',
    hint: 'Встроенный бесплатный ИИ. Ключ не нужен.',
    model: '',
    keyHint: '',
  },
  {
    id: 'openai',
    name: 'OpenAI',
    hint: 'GPT-4o, GPT-4o mini и другие модели OpenAI.',
    model: 'gpt-4o-mini',
    keyHint: 'sk-…',
  },
  {
    id: 'openrouter',
    name: 'OpenRouter',
    hint: 'Сотни моделей от разных разработчиков по одному ключу.',
    model: 'openrouter/auto',
    keyHint: 'sk-or-v1-…',
  },
  {
    id: 'groq',
    name: 'Groq',
    hint: 'Очень быстрые открытые модели: Llama, Mixtral, Gemma.',
    model: 'llama-3.3-70b-versatile',
    keyHint: 'gsk_…',
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    hint: 'DeepSeek Chat и Reasoner.',
    model: 'deepseek-chat',
    keyHint: 'sk-…',
  },
  {
    id: 'gemini',
    name: 'Google Gemini',
    hint: 'Gemini 2.0 Flash и другие модели Google.',
    model: 'gemini-2.0-flash',
    keyHint: 'AIza…',
  },
  {
    id: 'anthropic',
    name: 'Anthropic Claude',
    hint: 'Claude 3.5 Sonnet, Haiku и Opus.',
    model: 'claude-3-5-sonnet-latest',
    keyHint: 'sk-ant-…',
  },
  {
    id: 'custom',
    name: 'Свой сервер',
    hint: 'Любой OpenAI-совместимый API: Ollama, LM Studio, vLLM, прокси.',
    model: 'gpt-4o-mini',
    keyHint: 'Необязательно',
  },
];

/** Настройки ИИ: провайдер, ключ, модель и лимит ИИ-сборки. */
export function AiSettings() {
  const s = useStore();
  const t = useT();
  const c = s.config;
  const [aiModels, setAiModels] = useState<string[]>([]);
  const [aiKey, setAiKey] = useState(c.aiApiKey);
  const [aiModelText, setAiModelText] = useState(c.aiModel);
  const [aiBaseText, setAiBaseText] = useState(c.aiBaseUrl);
  const [aiBusy, setAiBusy] = useState(false);
  const aiProvider = AI_PROVIDERS.find((p) => p.id === c.aiProvider) ?? AI_PROVIDERS[0];
  const set = (patch: Partial<Config>) => s.setConfig({ ...c, ...patch });

  // Список моделей провайдера: грузится при открытии настроек и после смены провайдера.
  const loadAiModels = async (): Promise<string[]> => {
    try {
      const list = await api.aiModels();
      setAiModels(list);
      return list;
    } catch {
      setAiModels([]);
      return [];
    }
  };

  useEffect(() => {
    void loadAiModels();
  }, []);

  /** Провайдер сохраняем сразу, затем подтягиваем его модели. */
  const setProvider = async (id: string) => {
    const changed = id !== c.aiProvider;
    // Ключи у провайдеров разные: старый ключ не переносим, иначе запрос
    // уйдёт с чужим ключом и вернёт 401.
    set({
      aiProvider: id,
      aiModel: '',
      aiPackModel: '',
      ...(changed ? { aiApiKey: '' } : {}),
    });
    if (changed) setAiKey('');
    setAiModelText('');
    await flushConfig();
    await loadAiModels();
  };

  const reloadModels = async () => {
    setAiBusy(true);
    try {
      await flushConfig();
      const list = await loadAiModels();
      useStore.getState().toastMsg(
        list.length
          ? t('Доступно моделей: {n}', { n: list.length })
          : t('Список моделей недоступен — можно ввести вручную'),
      );
    } finally {
      setAiBusy(false);
    }
  };

  const testAi = async () => {
    setAiBusy(true);
    try {
      await flushConfig();
      const reply = await api.aiTest();
      useStore.getState().toastMsg(t('ИИ ответил: {r}', { r: reply }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setAiBusy(false);
    }
  };

  return (
    <>
      <div className="sw">
        <span>{t('ИИ-ассистент сборок')}</span>
        <Toggle on={c.aiBuilder} onClick={() => set({ aiBuilder: !c.aiBuilder })} />
      </div>
      <div className="hint" style={{ paddingBottom: 12 }}>
        {t('Кнопка «ИИ-сборка» в «Установках»: ассистент подбирает моды, ресурс-паки и шейдеры по описанию и сам скачивает их в новый профиль.')}
      </div>
      <div className="ai-box">
        <div className="ai-box-head">
          <b>{t('ИИ-провайдер')}</b>
          <span>{t(aiProvider.hint)}</span>
        </div>
        <div className="sw">
          <span>{t('Провайдер')}</span>
          <select
            style={{ width: 260 }}
            value={c.aiProvider}
            onChange={(e) => void setProvider(e.target.value)}
          >
            {AI_PROVIDERS.map((p) => (
              <option key={p.id} value={p.id}>
                {t(p.name)}
              </option>
            ))}
          </select>
        </div>
        <div className="hint" style={{ padding: '0 0 4px' }}>
          {t('Провайдер используется для ИИ-сборки, разбора крашей и проверки зависимостей.')}
        </div>
        {c.aiProvider === 'opencode' ? (
          <div className="hint" style={{ padding: '2px 0 8px' }}>
            {t('Встроенный бесплатный ИИ OpenCode Zen: ключ и регистрация не нужны.')}
          </div>
        ) : (
          <>
            <div className="sw">
              <span>{t('API-ключ')}</span>
              <input
                type="password"
                style={{ width: 260 }}
                value={aiKey}
                placeholder={t(aiProvider.keyHint)}
                onFocus={(e) => e.currentTarget.select()}
                onChange={(e) => setAiKey(e.target.value)}
                onBlur={() => set({ aiApiKey: aiKey })}
              />
            </div>
            {c.aiProvider === 'custom' && (
              <div className="sw">
                <span>{t('Base URL')}</span>
                <input
                  type="url"
                  style={{ width: 260 }}
                  value={aiBaseText}
                  placeholder="https://api.example.com/v1"
                  onChange={(e) => setAiBaseText(e.target.value)}
                  onBlur={() => set({ aiBaseUrl: aiBaseText.trim() })}
                />
              </div>
            )}
            <div className="sw">
              <span>{t('Модель')}</span>
              <input
                list="ai-model-list"
                style={{ width: 260 }}
                value={aiModelText}
                placeholder={aiProvider.model}
                onChange={(e) => setAiModelText(e.target.value)}
                onBlur={() => set({ aiModel: aiModelText.trim() })}
              />
              <datalist id="ai-model-list">
                {aiModels.map((m) => (
                  <option key={m} value={m} />
                ))}
              </datalist>
              <button
                type="button"
                className="btn sm"
                disabled={aiBusy}
                title={t('Обновить список моделей')}
                onClick={() => void reloadModels()}
              >
                ↻
              </button>
            </div>
            <div className="ai-box-actions">
              <button type="button" className="btn sm" disabled={aiBusy} onClick={() => void testAi()}>
                {aiBusy ? t('Проверяем…') : t('Проверить подключение')}
              </button>
              <span className="hint">
                {t('Ключ хранится в config.json и не показывается в интерфейсе. Пустая модель — дефолт провайдера.')}
              </span>
            </div>
          </>
        )}
      </div>
      <div className="sw" style={{ opacity: c.aiBuilder ? 1 : 0.45 }}>
        <span>{t('Лимит модов')}</span>
        <input
          type="number"
          min={1}
          max={200}
          style={{ width: 140 }}
          disabled={!c.aiBuilder}
          placeholder={t('Без ограничения')}
          value={c.aiPackLimit > 0 ? c.aiPackLimit : ''}
          onChange={(e) => {
            const n = parseInt(e.target.value, 10);
            set({ aiPackLimit: Number.isFinite(n) && n > 0 ? Math.min(200, n) : 0 });
          }}
        />
      </div>
      <div className="sw" style={{ opacity: c.aiBuilder ? 1 : 0.45 }}>
        <span>{t('Модель ИИ-сборки')}</span>
        <select
          style={{ width: 260 }}
          disabled={!c.aiBuilder}
          value={c.aiPackModel}
          onChange={(e) => set({ aiPackModel: e.target.value })}
        >
          <option value="">
            {c.aiProvider === 'opencode' ? t('Первая доступная') : t('По умолчанию')}
          </option>
          {[...new Set([...aiModels, ...(c.aiPackModel ? [c.aiPackModel] : [])])].map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>
      <div className="hint" style={{ paddingBottom: 12 }}>
        {t('Лимит: сколько модов максимум подбирать — пусто значит «сколько нужно сборке». Модель выбирается здесь, источник — в окне «ИИ-сборка».')}
      </div>
    </>
  );
}
