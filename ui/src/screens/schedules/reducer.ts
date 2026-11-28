// 日程屏纯函数 reducer（手册 C4.4 / 5.7）：日程列表 + 农历输入 + 插件闹钟降级态 + 五态角标（7.4）。
// 能力来自 gen/capabilities；错误只按 error.code 的 name 分支，不写三位数字字面量比较。

import { ERROR_CODES } from '../../gen/error-codes';
import { CAPABILITIES, type Role } from '../../gen/capabilities';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';

export interface UiOccurrence {
  readonly id: string;
  readonly title: string;
  readonly startsAt: string;
  readonly kind: string;
  readonly lunar: boolean;
}

/** 精确闹钟授权状态：被拒要记录降级，绝不假装准时（手册 6.4）。 */
export interface AlarmCapability {
  readonly exactAllowed: boolean;
  readonly mode: 'exact' | 'setAndAllowWhileIdle';
  readonly degraded: string | null;
}

export interface SchedulesViewState {
  readonly items: readonly UiOccurrence[];
  readonly role: Role;
  readonly alarm: AlarmCapability;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
  readonly lunarError: boolean;
}

export type SchedulesAction =
  | { readonly type: 'hydrate'; readonly items: readonly UiOccurrence[] }
  | { readonly type: 'setRole'; readonly role: Role }
  | { readonly type: 'alarmCapability'; readonly exactAllowed: boolean }
  | { readonly type: 'lunarInvalid' }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialSchedulesState(): SchedulesViewState {
  return {
    items: [],
    role: 'GUEST',
    alarm: { exactAllowed: false, mode: 'setAndAllowWhileIdle', degraded: null },
    badge: 'synced',
    errorCode: null,
    lunarError: false,
  };
}

export function canCreateSchedule(role: Role): boolean {
  return CAPABILITIES['create_schedules']?.includes(role) ?? false;
}

export function canReadSchedules(role: Role): boolean {
  return CAPABILITIES['read_schedules']?.includes(role) ?? false;
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

/** 精确闹钟授权映射：被拒时降级到 setAndAllowWhileIdle 并记 degraded（不假装准时）。 */
export function resolveAlarm(exactAllowed: boolean): AlarmCapability {
  return exactAllowed
    ? { exactAllowed: true, mode: 'exact', degraded: null }
    : {
        exactAllowed: false,
        mode: 'setAndAllowWhileIdle',
        degraded: 'EXACT_ALARM_DENIED / cosmetic / 提醒可能延迟至多 15 分钟',
      };
}

/** 农历输入校验：1900–2100 且月 1..12、日 1..30。越界返回 4005 语义。 */
export function validateLunar(year: number, month: number, day: number): boolean {
  return (
    Number.isInteger(year) &&
    Number.isInteger(month) &&
    Number.isInteger(day) &&
    year >= 1900 &&
    year <= 2100 &&
    month >= 1 &&
    month <= 12 &&
    day >= 1 &&
    day <= 30
  );
}

export function schedulesReducer(
  state: SchedulesViewState,
  action: SchedulesAction,
): SchedulesViewState {
  switch (action.type) {
    case 'hydrate':
      return { ...state, items: [...action.items], badge: 'synced', errorCode: null };
    case 'setRole':
      return { ...state, role: action.role };
    case 'alarmCapability':
      return { ...state, alarm: resolveAlarm(action.exactAllowed) };
    case 'lunarInvalid':
      return {
        ...state,
        lunarError: true,
        badge: 'error',
        errorCode: ERROR_CODES[4005].code,
      };
    case 'offline':
      return { ...state, badge: 'offline' };
    case 'gap':
      return { ...state, badge: 'gap' };
    case 'error':
      return {
        ...state,
        badge: badgeForFailure(action.code),
        errorCode: action.code,
        lunarError: action.code === ERROR_CODES[4005].code,
      };
    case 'synced':
      return { ...state, badge: 'synced', errorCode: null, lunarError: false };
  }
}
