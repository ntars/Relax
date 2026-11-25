export interface ChatMessage {
    id: string;
    from: string;
    to: string;
    content: string;
    timestamp: number;
    read: boolean;
}

export interface Announcement {
    id: string;
    title: string;
    content: string;
    created_at: string;
    read: boolean;
}

export interface FileEntry {
    id: string;
    name: string;
    size: number;
    path: string;
    created_at: string;
}

export interface Photo {
    id: string;
    url: string;
    date: string;
    album_id?: string;
}

export interface Schedule {
    id: string;
    title: string;
    description: string;
    start_at: string;
    end_at?: string;
    rrule?: string;
}

export interface Alarm {
    id: string;
    schedule_id: string;
    trigger_at: string;
    acknowledged: boolean;
}
