import { Routes, Route, Navigate } from 'react-router-dom'
import { useAuth } from './lib/auth'
import LoginPage from './pages/LoginPage'
import DashboardPage from './pages/DashboardPage'
import EndpointsPage from './pages/EndpointsPage'
import ProvidersPage from './pages/ProvidersPage'
import ApiKeysPage from './pages/ApiKeysPage'
import UsersPage from './pages/UsersPage'
import UsageMyPage from './pages/UsageMyPage'
import UsageSharedPage from './pages/UsageSharedPage'
import Layout from './components/Layout'

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { token } = useAuth()
  if (!token) return <Navigate to="/login" replace />
  return <>{children}</>
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/" element={<ProtectedRoute><Layout /></ProtectedRoute>}>
        <Route index element={<Navigate to="/dashboard" replace />} />
        <Route path="dashboard" element={<DashboardPage />} />
        <Route path="endpoints" element={<EndpointsPage />} />
        <Route path="providers" element={<ProvidersPage />} />
        <Route path="endpoints/:id/keys" element={<ApiKeysPage />} />
        <Route path="users" element={<UsersPage />} />
        <Route path="usage/my" element={<UsageMyPage />} />
        <Route path="usage/shared" element={<UsageSharedPage />} />
      </Route>
    </Routes>
  )
}
