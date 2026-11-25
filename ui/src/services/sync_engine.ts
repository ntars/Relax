import { SyncState } from '../types';

export class SyncEngine {
    private state: SyncState = SyncState.Idle;
    
    async step(): Promise<void> {
        this.state = SyncState.Syncing;
        
        // 模拟同步
        await new Promise(resolve => setTimeout(resolve, 100));
        
        this.state = SyncState.Completed;
    }
    
    getState(): SyncState {
        return this.state;
    }
}
