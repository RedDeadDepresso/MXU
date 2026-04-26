/**
 * useKkafioLogger
 *
 * Listens to `kkafio-output` Tauri events (or WebSocket in browser mode)
 * and feeds each line into the active instance's LogsPanel.
 *
 * KKAFIO's Logger formats every line as:
 *   <STATUS_PADDED> | <CATEGORY> | <message>
 *
 * Where STATUS_PADDED is the status keyword left-padded to 8 chars:
 *   "INFO    " | "SUCCESS " | "ERROR   " | "WARNING " |
 *   "SKIPPED " | "REPLACED" | "RENAMED " | "REMOVED "
 *
 * Separator lines are exactly:
 *   "--------------------------------------------------------------------"
 *
 * Color mapping (mirrors Logger.statusColor):
 *   INFO              → #2d8cf0  (blue)   → LogType 'info'
 *   SUCCESS           → #00c12b  (green)  → LogType 'success'
 *   ERROR             → #ed3f14  (red)    → LogType 'error'
 *   WARNING / SKIPPED / REPLACED / RENAMED / REMOVED → #f90 (orange) → LogType 'warning'
 *   separator / other → LogType 'agent'   (muted)
 */

import { useEffect, useRef } from 'react';
import { useAppStore, type LogType } from '@/stores/appStore';
import { loggers } from '@/utils/logger';
import { isTauri } from '@/utils/paths';
import * as wsService from '@/services/wsService';

const log = loggers.app;

// Padded status prefixes exactly as KKAFIO's Logger.align() produces them.
// align() pads to 8 chars with trailing spaces.
const STATUS_MAP: Array<[string, LogType]> = [
  ['SUCCESS ', 'success'],   // 8 chars: "SUCCESS "
  ['ERROR   ', 'error'],     // 8 chars: "ERROR   "
  ['WARNING ', 'warning'],   // 8 chars: "WARNING "
  ['SKIPPED ', 'warning'],   // 8 chars: "SKIPPED "
  ['REPLACED', 'warning'],   // 8 chars: "REPLACED"
  ['RENAMED ', 'warning'],   // 8 chars: "RENAMED "
  ['REMOVED ', 'warning'],   // 8 chars: "REMOVED "
  ['INFO    ', 'info'],      // 8 chars: "INFO    "  — last so SUCCESS isn't shadowed
];

const SEPARATOR = '--------------------------------------------------------------------';

/**
 * Classify a raw CLI stdout line into a LogType.
 * Checks whether each 8-char padded keyword appears anywhere in the line
 * (same logic as Logger.colorize — `if s in adding`).
 */
function classifyLine(line: string): LogType {
  if (line === SEPARATOR || line.trim() === '') return 'agent';
  for (const [prefix, type] of STATUS_MAP) {
    if (line.includes(prefix)) return type;
  }
  return 'agent';
}

export function useKkafioLogger() {
  const { addLog } = useAppStore();
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;

    const handleLine = (stream: string, line: string) => {
      if (cancelled) return;

      const instanceId = useAppStore.getState().activeInstanceId ?? '_kkafio_';

      let logType: LogType;
      if (stream === 'stderr') {
        // Stderr: use content-based coloring too; most stderr lines are errors
        logType = line.includes('ERROR   ') ? 'error'
                : line.includes('WARNING ') ? 'warning'
                : 'warning';
      } else {
        logType = classifyLine(line);
      }

      addLog(instanceId, { type: logType, message: line });
    };

    const setupListener = async () => {
      try {
        if (isTauri()) {
          const { listen } = await import('@tauri-apps/api/event');
          const unlisten = await listen<{ stream: string; line: string }>(
            'kkafio-output',
            (event) => {
              const { stream, line } = event.payload;
              handleLine(stream, line);
            },
          );
          if (cancelled) {
            unlisten();
          } else {
            unlistenRef.current = unlisten;
          }
        } else {
          const unlisten = wsService.onKkafioOutput(handleLine);
          if (cancelled) {
            unlisten();
          } else {
            unlistenRef.current = unlisten;
          }
        }
      } catch (err) {
        log.warn('Failed to setup kkafio output listener:', err);
      }
    };

    setupListener();

    return () => {
      cancelled = true;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [addLog]);
}
