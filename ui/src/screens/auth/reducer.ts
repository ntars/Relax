// 登录/绑定/TOTP 屏的纯函数状态机（手册 8.4「每屏一个 useReducer，状态枚举写死在卡里」）。
// 只按 error.code 对应的契约名分支，不读 message（手册 3.2 硬规则）。
import { ERROR_CODES } from '../../gen/error-codes';

export type AuthStage = 'credentials' | 'totp' | 'bind' | 'done' | 'error';

export interface AuthState {
  readonly stage: AuthStage;
  readonly errorCode: number | null;
  readonly busy: boolean;
}

export type AuthAction =
  | { readonly type: 'submit' }
  | { readonly type: 'error'; readonly code: number }
  | { readonly type: 'success' }
  | { readonly type: 'reset' };

export function initialAuthState(): AuthState {
  return { stage: 'credentials', errorCode: null, busy: false };
}

function errorName(code: number): string | undefined {
  return ERROR_CODES[code as keyof typeof ERROR_CODES]?.name;
}

export function authReducer(state: AuthState, action: AuthAction): AuthState {
  switch (action.type) {
    case 'submit':
      return { stage: state.stage, errorCode: null, busy: true };
    case 'error': {
      const name = errorName(action.code);
      if (name === ERROR_CODES[1003].name) {
        return { stage: 'totp', errorCode: action.code, busy: false };
      }
      if (name === ERROR_CODES[1008].name || name === ERROR_CODES[1005].name) {
        return { stage: 'bind', errorCode: action.code, busy: false };
      }
      return { stage: 'error', errorCode: action.code, busy: false };
    }
    case 'success':
      return { stage: 'done', errorCode: null, busy: false };
    case 'reset':
      return initialAuthState();
  }
}
