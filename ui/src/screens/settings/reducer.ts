// 设置屏纯函数 reducer（C5.5）：回收站（OWNER 清空/恢复）+ 审计只读页（OWNER）+ 能力控制 + 五态角标。
// 能力来自 gen/capabilities；错误只按 error.code 的 name 分支，不写三位数字字面量比较。
// 客户端隐藏按钮只是体验，不是安全边界——服务端中间件才是唯一判定点。

import { ERROR_CODES } from '../../gen/error-codes';
import { CAPABILITIES, type Role } from '../../gen/capabilities';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface UiRecycleItem {
  readonly id: string;
  readonly name: string;
  readonly recycledAt: string;
}

export interface UiAuditEntry {
  readonly id: number;
  readonly actorId: string;
  readonly action: string;
  readonly targetType: string;
  readonly targetId: string;
  readonly createdAt: string;
}

export interface SettingsViewState {
  readonly recycle: readonly UiRecycleItem[];
  readonly audit: readonly UiAuditEntry[];
  readonly role: Role;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
  readonly lastAction: string | null;
}

export type SettingsAction =
  | { readonly type: 'setRole'; readonly role: Role }
  | { readonly type: 'recycleLoaded'; readonly items: readonly UiRecycleItem[] }
  | { readonly type: 'auditLoaded'; readonly entries: readonly UiAuditEntry[] }
  | { readonly type: 'restored'; readonly id: string }
  | { readonly type: 'cleared' }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialSettingsState(): SettingsViewState {
  return {
    recycle: [],
    audit: [],
    role: 'GUEST',
    badge: 'synced',
    errorCode: null,
    lastAction: null,
  };
}

export function canClearRecycleBin(role: Role): boolean {
  return CAPABILITIES['clear_recycle_bin']?.includes(role) ?? false;
}
export function canRestoreAnyFile(role: Role): boolean {
  return CAPABILITIES['restore_any_file']?.includes(role) ?? false;
}
export function canReadAuditLog(role: Role): boolean {
  return CAPABILITIES['read_audit_log']?.includes(role) ?? false;
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

function badgeForFailure(code: number): BadgeState {
  const name = errorName(code);
  if (
    name === ERROR_CODES[3001].name ||
    name === ERROR_CODES[3002].name ||
    name === ERROR_CODES[3003].name
  ) {
    return 'offline';
  }
  if (name === ERROR_CODES[3004].name) {
    return 'gap';
  }
  return 'error';
}

export function settingsReducer(state: SettingsViewState, action: SettingsAction): SettingsViewState {
  switch (action.type) {
    case 'setRole':
      return { ...state, role: action.role };
    case 'recycleLoaded':
      return { ...state, recycle: [...action.items], badge: 'synced', errorCode: null };
    case 'auditLoaded':
      return { ...state, audit: [...action.entries] };
    case 'restored':
      return {
        ...state,
        recycle: state.recycle.filter((it) => it.id !== action.id),
        lastAction: '已恢复',
        badge: 'synced',
      };
    case 'cleared':
      return { ...state, recycle: [], lastAction: '回收站已清空', badge: 'synced' };
    case 'offline':
      return { ...state, badge: 'offline' };
    case 'gap':
      return { ...state, badge: 'gap' };
    case 'error':
      return { ...state, badge: badgeForFailure(action.code), errorCode: action.code };
    case 'synced':
      return { ...state, badge: 'synced', errorCode: null };
  }
}
