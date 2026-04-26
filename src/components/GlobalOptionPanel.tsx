/**
 * GlobalOptionPanel
 *
 * Collapsible, pinned panel at the top of the task list that renders every
 * option listed in `projectInterface.global_option` (currently just GamePath).
 *
 * Values are stored on the instance as `globalOptionValues` (not per-task)
 * so they're always persisted independently of which tasks are enabled.
 */

import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings2, ChevronDown, ChevronRight } from 'lucide-react';
import { toast } from 'sonner';
import clsx from 'clsx';
import { useAppStore } from '@/stores/appStore';
import { getInterfaceLangKey } from '@/i18n';
import { resolveI18nText } from '@/services/contentResolver';
import { createDefaultOptionValue } from '@/stores/helpers';
import type { OptionValue, FolderOption } from '@/types/interface';

// ── Dropdown menu ────────────────────────────────────────────────────────────

interface MenuItem {
  label: string;
  disabled?: boolean;
  onClick: () => void;
}

function DropdownMenu({ items }: { items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, [open]);

  return (
    <div className="relative flex-shrink-0" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex items-center px-2.5 py-1.5 rounded-md bg-bg-tertiary border border-border text-sm text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors select-none"
        aria-haspopup="menu"
        aria-expanded={open}
      >
        ⋯
      </button>
      {open && (
        <div className="absolute right-0 mt-1 z-50 min-w-[170px] py-1 rounded-lg border border-border bg-bg-secondary shadow-lg">
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              disabled={item.disabled}
              onClick={() => { setOpen(false); item.onClick(); }}
              className={clsx(
                'w-full text-left px-3 py-1.5 text-sm transition-colors',
                item.disabled
                  ? 'text-text-muted cursor-not-allowed'
                  : 'text-text-primary hover:bg-bg-hover',
              )}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export function GlobalOptionPanel({ instanceId }: { instanceId: string }) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);

  const {
    projectInterface,
    instances,
    setGlobalOptionValue,
    language,
    interfaceTranslations,
  } = useAppStore();

  const instance   = instances.find((i) => i.id === instanceId);
  const globalOpts = projectInterface?.global_option ?? [];

  if (globalOpts.length === 0 || !projectInterface?.option) return null;

  const langKey     = getInterfaceLangKey(language);
  const translations = interfaceTranslations[langKey];
  const globalOptionValues = instance?.globalOptionValues ?? {};

  return (
    <div className="rounded-lg border border-accent/30 bg-bg-secondary shadow-sm overflow-visible">

      {/* ── Header (clickable to collapse) ───────────────────────────── */}
      <button
        type="button"
        onClick={() => setCollapsed((c) => !c)}
        className="w-full flex items-center gap-2 px-3 py-2 bg-accent/5 border-b border-accent/20 hover:bg-accent/10 transition-colors rounded-t-lg"
        aria-expanded={!collapsed}
      >
        <Settings2 className="w-3.5 h-3.5 text-accent flex-shrink-0" />
        <span className="flex-1 text-left text-xs font-semibold text-accent tracking-wide uppercase">
          {t('globalOptions.title', 'Global Settings')}
        </span>
        {collapsed
          ? <ChevronRight className="w-3.5 h-3.5 text-accent/60" />
          : <ChevronDown  className="w-3.5 h-3.5 text-accent/60" />}
      </button>

      {/* ── Options ──────────────────────────────────────────────────── */}
      {!collapsed && (
        <div className="p-3 space-y-3">
          {globalOpts.map((optionKey) => {
            const optionDef = projectInterface.option![optionKey];
            if (!optionDef) return null;

            const currentValue: OptionValue =
              globalOptionValues[optionKey] ?? createDefaultOptionValue(optionDef);

            const label = resolveI18nText(
              (optionDef as { label?: string }).label, translations,
            ) || optionKey;

            const description = resolveI18nText(
              (optionDef as { description?: string }).description, translations,
            );

            const setVal = (v: OptionValue) => setGlobalOptionValue(instanceId, optionKey, v);

            // ── Folder ──
            if (optionDef.type === 'folder') {
              const folderDef  = optionDef as FolderOption;
              const folderPath = currentValue.type === 'folder'
                ? currentValue.path
                : (folderDef.default ?? '');

              const setPath = (p: string) => setVal({ type: 'folder', path: p });

              const handleBrowse = async () => {
                try {
                  const { open } = await import('@tauri-apps/plugin-dialog');
                  const selected = await open({ directory: true, multiple: false });
                  if (typeof selected === 'string' && selected) setPath(selected);
                } catch { /* cancelled */ }
              };

              const handleShowInExplorer = async () => {
                if (!folderPath) return;
                try {
                  const { invoke } = await import('@tauri-apps/api/core');
                  await invoke('open_file', { filePath: folderPath });
                } catch { /* ignore */ }
              };

              const handleRunGame = async () => {
                if (!folderPath) return;
                try {
                  const { invoke } = await import('@tauri-apps/api/core');
                  const result = await invoke<{ ok: boolean; exe: string; error: string }>(
                    'kkafio_run_game',
                    { gamePath: folderPath },
                  );
                  if (result.ok) {
                    const exeName = result.exe.replace(/\\/g, '/').split('/').pop() ?? result.exe;
                    toast.success(`Launched ${exeName}`);
                  } else {
                    toast.error(result.error);
                  }
                } catch (e) {
                  toast.error(`Failed to launch game: ${e}`);
                }
              };

              const menuItems: MenuItem[] = [
                {
                  label: t('options.folder.showInExplorer', 'Show in Explorer'),
                  disabled: !folderPath,
                  onClick: handleShowInExplorer,
                },
                {
                  label: t('options.folder.browse', 'Browse…'),
                  onClick: handleBrowse,
                },
                {
                  label: t('options.gameFolder.run', 'Run Game'),
                  disabled: !folderPath,
                  onClick: handleRunGame,
                },
              ];

              return (
                <div key={optionKey} className="space-y-1">
                  <div>
                    <p className="text-sm font-medium text-text-primary">{label}</p>
                    {description && (
                      <p className="text-xs text-text-muted mt-0.5">{description}</p>
                    )}
                  </div>
                  <div className="flex gap-2 items-center">
                    <input
                      type="text"
                      value={folderPath}
                      onChange={(e) => setPath(e.target.value)}
                      placeholder={folderDef.placeholder ?? t('options.folder.placeholder', 'Select a folder…')}
                      className="flex-1 min-w-0 px-3 py-1.5 rounded-md bg-bg-primary border border-border text-sm text-text-primary placeholder:text-text-muted focus:outline-none focus:ring-1 focus:ring-accent font-mono"
                    />
                    <DropdownMenu items={menuItems} />
                  </div>
                </div>
              );
            }

            return null;
          })}
        </div>
      )}
    </div>
  );
}
