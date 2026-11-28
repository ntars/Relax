// 认证流程顶层容器：authReducer 决定渲染哪屏；各屏一个自己的 useReducer 管表单。
import { useReducer, useState } from 'react';
import { ERROR_CODES } from '../../gen/error-codes';
import type { AuthToken } from '../../services/api';
import { BindScreen } from './BindScreen';
import { CredentialsScreen } from './CredentialsScreen';
import type { Credentials } from './CredentialsScreen';
import { authReducer, initialAuthState } from './reducer';
import { TotpScreen } from './TotpScreen';

export function AuthScreen() {
  const [state, dispatch] = useReducer(authReducer, undefined, initialAuthState);
  const [credentials, setCredentials] = useState<Credentials>({ memberId: '', password: '', deviceId: '' });

  const onSuccess = (_token: AuthToken) => {
    dispatch({ type: 'success' });
  };

  const onFailure = (code: number, creds?: Credentials) => {
    if (creds !== undefined) {
      setCredentials(creds);
    }
    dispatch({ type: 'error', code });
  };

  const errorMessage = state.errorCode === null ? '' : (ERROR_CODES[state.errorCode as keyof typeof ERROR_CODES]?.message ?? '认证失败');

  return (
    <div className="auth-flow">
      {state.stage === 'credentials' && (
        <CredentialsScreen initial={credentials} onSuccess={onSuccess} onFailure={onFailure} />
      )}
      {state.stage === 'totp' && (
        <TotpScreen credentials={credentials} onSuccess={onSuccess} onFailure={onFailure} />
      )}
      {state.stage === 'bind' && (
        <BindScreen onSuccess={onSuccess} onFailure={onFailure} />
      )}
      {state.stage === 'error' && (
        <div className="auth-error">
          <p>{errorMessage}</p>
          <button type="button" onClick={() => dispatch({ type: 'reset' })}>返回登录</button>
        </div>
      )}
      {state.stage === 'done' && <div className="auth-done">已登录</div>}
    </div>
  );
}
