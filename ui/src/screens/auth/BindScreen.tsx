// 设备绑定屏：设备被撤销或邀请码无效时，重新入网/绑定。invoke only。
import { useReducer } from 'react';
import { bindDevice, enroll, errorCodeOf } from '../../services/api';
import type { AuthToken } from '../../services/api';

interface BindForm {
  inviteCode: string;
  name: string;
  deviceId: string;
  deviceLabel: string;
  password: string;
}

type Field = keyof BindForm;
interface FormAction {
  field: Field;
  value: string;
}

function bindReducer(state: BindForm, action: FormAction): BindForm {
  return { ...state, [action.field]: action.value };
}

interface Props {
  onSuccess: (token: AuthToken) => void;
  onFailure: (code: number) => void;
}

export function BindScreen({ onSuccess, onFailure }: Props) {
  const [form, dispatch] = useReducer(bindReducer, {
    inviteCode: '',
    name: '',
    deviceId: '',
    deviceLabel: '',
    password: '',
  });

  const submitEnroll = async () => {
    try {
      onSuccess(await enroll(form));
    } catch (error) {
      onFailure(errorCodeOf(error) ?? 0);
    }
  };

  const submitBind = async () => {
    try {
      await bindDevice(form.deviceId, form.deviceLabel);
      onFailure(0);
    } catch (error) {
      onFailure(errorCodeOf(error) ?? 0);
    }
  };

  return (
    <div className="auth-screen auth-bind">
      <h2>绑定设备</h2>
      <label>邀请码<input value={form.inviteCode} onChange={(e) => dispatch({ field: 'inviteCode', value: e.target.value })} /></label>
      <label>姓名<input value={form.name} onChange={(e) => dispatch({ field: 'name', value: e.target.value })} /></label>
      <label>设备 ID<input value={form.deviceId} onChange={(e) => dispatch({ field: 'deviceId', value: e.target.value })} /></label>
      <label>设备名<input value={form.deviceLabel} onChange={(e) => dispatch({ field: 'deviceLabel', value: e.target.value })} /></label>
      <label>密码<input type="password" value={form.password} onChange={(e) => dispatch({ field: 'password', value: e.target.value })} /></label>
      <button type="button" onClick={() => void submitEnroll()}>入网</button>
      <button type="button" onClick={() => void submitBind()}>绑定</button>
    </div>
  );
}
