import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import ReportCheckerPage from './pages/ReportCheckerPage';
import "./App.css";

function App() {
  return (
    <Router>
      <Layout>
        <Routes>
          <Route path="/" element={<ReportCheckerPage />} />
          <Route path="/report-checker" element={<ReportCheckerPage />} />
        </Routes>
      </Layout>
    </Router>
  );
}

export default App;
