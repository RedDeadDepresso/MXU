/**
 * taskStopService — KKAFIO edition
 *
 * Replaces the MaaFramework-based stop logic with a simple kkafio_stop invoke.
 */

import { loggers } from '@/utils/logger';
import { isTauri } from '@/utils/paths';
import { invoke } from '@tauri-apps/api/core';
import { cancelTaskQueueMonitor } from './taskMonitor';
import { useAppStore } from '@/stores/appStore';

const log = loggers.task;

function cleanupTaskState(instanceId: string) {
  const state = useAppStore.getState();
  state.updateInstance(instanceId, { isRunning: false });
  if ('setInstanceTaskStatus' in state) (state as any).setInstanceTaskStatus(instanceId, null);
  if ('setInstanceCurrentTaskId' in state) (state as any).setInstanceCurrentTaskId(instanceId, null);
  state.clearScheduleExecution(instanceId);
}

export async function stopInstanceTasks(instanceId: string): Promise<boolean> {
  log.info(`[task-stop#${instanceId}] stopping KKAFIO CLI`);
  try {
    await invoke('kkafio_stop');
    cancelTaskQueueMonitor(instanceId);
    cleanupTaskState(instanceId);
    return true;
  } catch (error) {
    log.error(`[task-stop#${instanceId}] kkafio_stop failed:`, error);
    return false;
  }
}

export async function stopInstanceTasksAndExitApp(instanceId: string): Promise<boolean> {
  const stopped = await stopInstanceTasks(instanceId);
  if (!stopped) return false;
  if (!isTauri()) return true;
  const { exit } = await import('@tauri-apps/plugin-process');
  await exit(0);
  return true;
}
