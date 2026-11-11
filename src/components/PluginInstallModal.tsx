// Проверка плагина до установки: права, сеть, обфускация, автор и код.
import { useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import type { PluginPreview } from '../lib/api';
import { useT } from '../lib/i18n';

const WARN_KEY: Record<string, string> = {
  network: 'Код обращается к внешним серверам.',
  network_permission: 'Запрошено право network — плагин может ходить в сеть.',
  fs_permission: 'Запрошено право fs — плагин может читать и писать файлы.',
  dangerous: 'Код может запускать программы или обращаться к системным API.',
  exec: 'Найден динамический код: exec/eval/compile.',
  base64: 'Найден длинный base64-блок — код может быть скрыт.',
  hex: 'Найдены длинные hex-последовательности — возможна обфускация.',
  long_lines: 'Очень длинные строки — код может быть минифицирован.',
  entropy: 'Высокая энтропия кода — возможна обфускация.',
  permissions: 'Плагин запрашивает права: {list}.',
  hooks: 'Плагин подписывается на хуки: {list}.',
  author_missing: 'Автор плагина не указан.',
  api_version: 'Плагин написан для API {api}, лаунчер поддерживает API 1.',
  binary_files: 'В плагине есть бинарные файлы — их содержимое не показано.',
  no_plugins: 'В архиве нет ни одного файла .aplug.',
};

function fmtSize(n: number): string {
  if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  if (n >= 1024) return `${Math.round(n / 1024)} KB`;
  return `${n} B`;
}

export function PluginInstallModal({
  preview,
  busy,
  onCancel,
  onInstall,
}: {
  preview: PluginPreview;
  busy: boolean;
  onCancel: () => void;
  onInstall: () => void;
}) {
  const t = useT();
  const codeFiles = useMemo(() => preview.files.filter((f) => f.code), [preview.files]);
  const [file, setFile] = useState(codeFiles[0]?.name ?? '');
  const current = codeFiles.find((f) => f.name === file) ?? codeFiles[0];
  const riskColor =
    preview.risk === 'high' ? '#ff8b80' : preview.risk === 'medium' ? '#ffd18f' : '#9ee493';
  const riskLabel =
    preview.risk === 'high'
      ? t('Высокий риск')
      : preview.risk === 'medium'
        ? t('Средний риск')
        : t('Низкий риск');

  const warnText = (w: { code: string; message: string }) => {
    const key = WARN_KEY[w.code];
    if (!key) return w.message;
    const list = preview.permissions.join(', ');
    return t(key, { list, api: preview.api });
  };

  return createPortal(
    <div className="ov" onClick={onCancel}>
      <div className="mod plugin-preview" style={{ width: 820 }} onClick={(e) => e.stopPropagation()}>
        <div className="plugin-preview-head">
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="plugin-preview-title">
              {preview.name}
              <span className="hint">v{preview.version}</span>
            </div>
            <div className="hint">
              {preview.id} · {preview.author || t('Автор не указан')} · API {preview.api}
            </div>
            {preview.description ? <div className="plugin-preview-desc">{preview.description}</div> : null}
          </div>
          <span className="plugin-preview-risk" style={{ color: riskColor, borderColor: riskColor }}>
            {riskLabel}
          </span>
        </div>

        <div className="plugin-preview-body">
          <div className="plugin-preview-meta">
            <span>
              {preview.fileName} · {fmtSize(preview.size)}
            </span>
            <span className="plugin-preview-hash" title={preview.sha256}>
              SHA-256: {preview.sha256.slice(0, 16)}…
            </span>
          </div>

          <div className="plugin-preview-badges">
            <span className={'plugin-preview-badge' + (preview.network ? ' on' : '')}>
              {preview.network ? t('Есть сетевые запросы') : t('Без сети')}
            </span>
            <span className={'plugin-preview-badge' + (preview.obfuscated ? ' on' : '')}>
              {preview.obfuscated ? t('Возможна обфускация') : t('Код читаемый')}
            </span>
            <span className={'plugin-preview-badge' + (preview.dangerous ? ' on' : '')}>
              {preview.dangerous ? t('Опасные вызовы') : t('Опасных вызовов нет')}
            </span>
          </div>

          <div className="plugin-preview-section">
            <b>{t('Права')}</b>
            <div className="plugin-preview-chips">
              {preview.permissions.length ? (
                preview.permissions.map((p) => (
                  <span key={p} className="plugin-preview-chip">
                    {p}
                  </span>
                ))
              ) : (
                <span className="hint">{t('Права не запрошены')}</span>
              )}
            </div>
          </div>

          {preview.warnings.length > 0 && (
            <div className="plugin-preview-section">
              <b>{t('Предупреждения')}</b>
              <div className="plugin-preview-warnings">
                {preview.warnings.map((w) => (
                  <div key={w.code} className={'plugin-preview-warn ' + w.level}>
                    <span>{warnText(w)}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="plugin-preview-section">
            <b>{t('Код плагина')}</b>
            {codeFiles.length > 1 && (
              <div className="plugin-preview-files">
                {codeFiles.map((f) => (
                  <button
                    key={f.name}
                    type="button"
                    className={'plugin-preview-file' + (f.name === current?.name ? ' on' : '')}
                    onClick={() => setFile(f.name)}
                  >
                    {f.name}
                  </button>
                ))}
              </div>
            )}
            {current?.code ? (
              <pre className="plugin-preview-code">{current.code}</pre>
            ) : (
              <div className="hint">{t('Нет текстовых файлов для показа.')}</div>
            )}
          </div>
        </div>

        <div className="plugin-preview-foot">
          <span className="hint" style={{ flex: 1 }}>
            {t('Плагины выполняются в процессе лаунчера — ставьте только те, которым доверяете.')}
          </span>
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            {t('Отмена')}
          </button>
          <button type="button" className="btn pr" onClick={onInstall} disabled={busy}>
            {busy ? t('Устанавливаю…') : t('Установить')}
          </button>
        </div>
      </div>
    </div>,
    document.getElementById('win') ?? document.body,
  );
}
