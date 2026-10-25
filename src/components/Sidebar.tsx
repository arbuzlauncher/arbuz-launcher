import { ACCOUNT_LABEL } from '../lib/labels';
import { useT } from '../lib/i18n';
import { Icon, type IconName } from '../lib/nav-icons';
import { MaterialIcon, type MaterialName } from '../lib/material-icons';
import { monetLike } from '../lib/theme';
import { curAccount, useStore, type View } from '../store';
import { Avatar } from './Controls';

const VIEW_ICON: Record<View, IconName> = {
  home: 'home',
  installs: 'folder',
  browse: 'modpacks',
  accounts: 'accounts',
  coop: 'coop',
  servers: 'server',
  app: 'settings',
  p: 'modpacks',
};

const MATERIAL_ICON: Record<View, MaterialName> = {
  home: 'home',
  installs: 'inventory_2',
  browse: 'extension',
  accounts: 'account_circle',
  coop: 'person_add',
  servers: 'dns',
  app: 'settings',
  p: 'extension',
};

export function Sidebar() {
  const s = useStore();
  const t = useT();
  const acc = curAccount(s);
  const monet = monetLike(s.config.style);

  const item = (view: View, label: string) => (
    <button
      key={view}
      type="button"
      className={'side-item' + (s.view === view ? ' on' : '')}
      onClick={() => s.set({ view, menu: false })}
    >
      <span className="side-icon">
        {monet ? <MaterialIcon name={MATERIAL_ICON[view]} size={22} /> : <Icon name={VIEW_ICON[view]} size={20} />}
      </span>
      {label}
    </button>
  );

  return (
    <div id="sb">
      <button type="button" className="side-acc" onClick={() => s.set({ menu: !s.menu })}>
        <Avatar name={acc ? acc.name : '?'} skin={acc ? s.skins[acc.id] : null} size={38} />
        <span style={{ flex: 1, minWidth: 0 }}>
          <b>{acc ? acc.name : t('Нет аккаунта')}</b>
          <small>{acc ? t(ACCOUNT_LABEL[acc.kind]) : t('Нажмите, чтобы добавить')}</small>
        </span>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>

      <div className="side-nav">
        {item('home', t('Главная'))}
        {item('installs', t('Установки'))}
        {item('browse', t('Каталог'))}
        {item('coop', t('Co-op'))}
        {item('servers', t('Серверы'))}
        {item('accounts', t('Аккаунты'))}
      </div>

      <div className="side-spacer" />

      <div className="side-nav">{item('app', t('Настройки'))}</div>
    </div>
  );
}
