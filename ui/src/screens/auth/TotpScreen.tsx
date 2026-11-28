// TOTP 二次验证屏：携带凭据屏采集的 memberId/password/deviceId 加验证码重登。
import { useReducer } from 'react';
import { errorCodeOf, loginRequest } from '../../services/api';
import type { AuthToken } from '../../services/api';
import type { Credentials } from './CredentialsScreen';

interface TotpForm {
  code: string;
}

type FormAction = { field: 'code'; value: string };

function totpReducer(state: TotpForm, action: FormAction): TotpForm {
  return { ...state, [action.field]: action.value };
}

interface Props {
  credentials: Credentials;
  onSuccess: (token: AuthToken) => void;
  onFailure: (code: number) => void;
}

export function TotpScreen({ credentials, onSuccess, onFailure }: Props) {
  const [form, dispatch] = useReducer(totpReducer, { code: '' });

  const submit = async () => {
    try {
      onSuccess(await loginRequest({ ...credentials, totp: form.code }));
    } catch (error) {
      onFailure(errorCodeOf(error) ?? 0);
    }
  };

  return (
    <div className="auth-screen auth-totp">
      <h2>二次验证</h2>
      <label>
        验证码
        <input value={form.code} onChange={(e) => dispatch({ field: 'code', value: e.target.value })} />
      </label>
      <button type="button" onClick={() => void submit()}>验证</button>
    </div>
  );
}
