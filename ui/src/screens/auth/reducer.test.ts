import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../gen/error-codes';
import { authReducer, initialAuthState } from './reducer';

describe('authReducer', () => {
  it('submit 置 busy', () => {
    const s = authReducer(initialAuthState(), { type: 'submit' });
    expect(s.busy).toBe(true);
    expect(s.errorCode).toBeNull();
  });

  it('TOTP_REQUIRED 切到 totp 阶段', () => {
    const code = ERROR_CODES[1003].code;
    const s = authReducer(initialAuthState(), { type: 'error', code });
    expect(s.stage).toBe('totp');
    expect(s.errorCode).toBe(code);
  });

  it('DEVICE_REVOKED / INVITE_INVALID 切到 bind 阶段', () => {
    expect(authReducer(initialAuthState(), { type: 'error', code: ERROR_CODES[1008].code }).stage).toBe('bind');
    expect(authReducer(initialAuthState(), { type: 'error', code: ERROR_CODES[1005].code }).stage).toBe('bind');
  });

  it('其它错误码进 error 阶段', () => {
    const s = authReducer(initialAuthState(), { type: 'error', code: ERROR_CODES[1002].code });
    expect(s.stage).toBe('error');
  });

  it('success 进 done 并清 busy', () => {
    const s = authReducer(initialAuthState(), { type: 'success' });
    expect(s.stage).toBe('done');
    expect(s.busy).toBe(false);
  });

  it('reset 回到初始态', () => {
    const done = authReducer(initialAuthState(), { type: 'success' });
    expect(authReducer(done, { type: 'reset' })).toEqual(initialAuthState());
  });
});
