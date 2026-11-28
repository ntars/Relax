// 作息屏纯函数 reducer（手册 C4.3 / 5.7）：家庭/个人切换 + 覆盖优先级 + 五态角标（7.4）。
// 能力来自 gen/capabilities，错误只按 error.code 的 name 分支，不写三位数字字面量比较。

import { ERROR_CODES } from '../../gen/error-codes';
import { CAPABILITIES, type Role } from '../../gen/capabilities';

export type BadgeState = 'synced' | 'pending' | 'offline' | 'gap' | 'error';
export type RoutineScope = 'family' | 'personal';

export interface UiRoutine {
  readonly id: string;
  readonly title: string;
  readonly kind: string;
  readonly daysMask: number;
  readonly startMinute: number;
  readonly endMinute: number;
  readonly active: boolean;
  readonly isFamily: boolean;
  readonly overridesFamily: boolean;
}

export interface RoutinesViewState {
  readonly scope: RoutineScope;
  readonly familyItems: readonly UiRoutine[];
  readonly personalItems: readonly UiRoutine[];
  readonly role: Role;
  readonly badge: BadgeState;
  readonly errorCode: number | null;
}

export type RoutinesAction =
  | { readonly type: 'hydrate'; readonly family: readonly UiRoutine[]; readonly personal: readonly UiRoutine[] }
  | { readonly type: 'setScope'; readonly scope: RoutineScope }
  | { readonly type: 'setRole'; readonly role: Role }
  | { readonly type: 'upsert'; readonly item: UiRoutine }
  | { readonly type: 'offline' }
  | { readonly type: 'gap' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'synced' };

export function initialRoutinesState(): RoutinesViewState {
  return {
    scope: 'family',
    familyItems: [],
    personalItems: [],
    role: 'GUEST',
    badge: 'synced',
    errorCode: null,
  };
}

export function canReadRoutines(role: Role): boolean {
  return CAPABILITIES['read_routines']?.includes(role) ?? false;
}

export function canManageFamily(role: Role): boolean {
  return CAPABILITIES['manage_family_routines']?.includes(role) ?? false;
}

export function canEditPersonal(role: Role): boolean {
  return CAPABILITIES['edit_personal_routine']?.includes(role) ?? false;
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

/** 当前 scope 可见的项：家庭视图=家庭全集，个人视图=个人全集（含覆盖标记）。 */
export function visibleItems(state: RoutinesViewState): readonly UiRoutine[] {
  return state.scope === 'family' ? state.familyItems : state.personalItems;
}

export function routinesReducer(state: RoutinesViewState, action: RoutinesAction): RoutinesViewState {
  switch (action.type) {
    case 'hydrate':
      return {
        ...state,
        familyItems: [...action.family],
        personalItems: [...action.personal],
        badge: 'synced',
        errorCode: null,
      };
    case 'setScope':
      return { ...state, scope: action.scope };
    case 'setRole':
      return { ...state, role: action.role };
    case 'upsert': {
      const item = action.item;
      const family = item.isFamily
        ? [...state.familyItems.filter((r) => r.id !== item.id), item]
        : state.familyItems;
      const personal = item.isFamily
        ? state.personalItems
        : [...state.personalItems.filter((r) => r.id !== item.id), item];
      return { ...state, familyItems: family, personalItems: personal };
    }
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
