import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import GDriveEditorPage from './pages/GDriveEditorPage';
import ReportCheckerPage from './pages/ReportCheckerPage';
import "./App.css";

function App() {
  return (
    <Router>
      <Layout>
        <Routes>
          <Route path="/" element={<Navigate to="/report-checker" replace />} />
          <Route path="/gdrive-editor" element={<GDriveEditorPage />} />
          <Route path="/report-checker" element={<ReportCheckerPage />} />
        </Routes>
      </Layout>
    </Router>
  );
}

export default App;
