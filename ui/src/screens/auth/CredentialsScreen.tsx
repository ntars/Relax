// 登录凭据屏：唯一跨进程通道是 api.ts 的 invoke 封装（invoke only）。
import { useReducer } from 'react';
import { errorCodeOf, loginRequest } from '../../services/api';
import type { AuthToken } from '../../services/api';

export interface Credentials {
  memberId: string;
  password: string;
  deviceId: string;
}

type Field = keyof Credentials;
interface FormAction {
  field: Field;
  value: string;
}

function formReducer(state: Credentials, action: FormAction): Credentials {
  return { ...state, [action.field]: action.value };
}

interface Props {
  initial: Credentials;
  onSuccess: (token: AuthToken) => void;
  onFailure: (code: number, creds: Credentials) => void;
}

export function CredentialsScreen({ initial, onSuccess, onFailure }: Props) {
  const [form, dispatch] = useReducer(formReducer, initial);

  const submit = async () => {
    try {
      onSuccess(await loginRequest(form));
    } catch (error) {
      onFailure(errorCodeOf(error) ?? 0, form);
    }
  };

  return (
    <div className="auth-screen auth-credentials">
      <h2>登录</h2>
      <label>
        成员 ID
        <input value={form.memberId} onChange={(e) => dispatch({ field: 'memberId', value: e.target.value })} />
      </label>
      <label>
        密码
        <input type="password" value={form.password} onChange={(e) => dispatch({ field: 'password', value: e.target.value })} />
      </label>
      <label>
        设备 ID
        <input value={form.deviceId} onChange={(e) => dispatch({ field: 'deviceId', value: e.target.value })} />
      </label>
      <button type="button" onClick={() => void submit()}>登录</button>
    </div>
  );
}
