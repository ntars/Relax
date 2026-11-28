import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { LoginPage } from './pages/LoginPage';
import { DashboardPage } from './pages/DashboardPage';
import { ChatPage } from './pages/ChatPage';
import { AnnouncementPage } from './pages/AnnouncementPage';
import { NewsPage } from './pages/NewsPage';
import { RoutinePage } from './pages/RoutinePage';
import { SchedulePage } from './pages/SchedulePage';
import { FilePage } from './pages/FilePage';
import { AlbumPage } from './pages/AlbumPage';

function App() {
    return (
        <BrowserRouter>
            <Routes>
                <Route path="/login" element={<LoginPage />} />
                <Route path="/dashboard" element={<DashboardPage />} />
                <Route path="/chat" element={<ChatPage />} />
                <Route path="/announcement" element={<AnnouncementPage />} />
                <Route path="/news" element={<NewsPage />} />
                <Route path="/routine" element={<RoutinePage />} />
                <Route path="/schedule" element={<SchedulePage />} />
                <Route path="/file" element={<FilePage />} />
                <Route path="/album" element={<AlbumPage />} />
                <Route path="/" element={<Navigate to="/login" replace />} />
            </Routes>
        </BrowserRouter>
    );
}

export default App;
