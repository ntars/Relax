import { SyncEngine } from './sync_engine';

export class SyncService {
    private engine: SyncEngine;
    
    constructor() {
        this.engine = new SyncEngine();
    }
    
    async syncNow(): Promise<void> {
        await this.engine.step();
    }
    
    async getStatus(): Promise<SyncStatus> {
        return { state: 'idle' };
    }
}

export interface SyncStatus {
    state: 'idle' | 'syncing' | 'completed' | 'failed';
    last_sync?: string;
}
