import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { jwtDecode } from "jwt-decode";
import { Link, useLocation } from "react-router-dom";
import SettingsDrawer from "./SettingsDrawer";

interface GoogleTokens {
  access_token: string;
  refresh_token: string;
  id_token: string;
  expires_in?: number;
  scope?: string;
  token_type?: string;
}

interface IdTokenPayload {
  name?: string;
  email?: string;
  [key: string]: any;
}

interface LayoutProps {
  children: React.ReactNode;
  openedFileName?: string | null;
}

export default function Layout({ children }: LayoutProps) {
  // Auth state
  const [isAuthenticated, setIsAuthenticated] = useState<boolean | null>(null);
  const [userName, setUserName] = useState<string>("");
  const [googleClientId, setGoogleClientId] = useState<string>("");
  const [showCodeInput, setShowCodeInput] = useState(false);
  const [authCode, setAuthCode] = useState("");
  const [authError, setAuthError] = useState<string | null>(null);
  const [isExchanging, setIsExchanging] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  
  const location = useLocation();

  // On mount, check authentication and get client ID
  useEffect(() => {
    async function checkAuth() {
      try {
        const idToken: string | null = await invoke("get_auth_state");
        if (idToken) {
          const decoded: IdTokenPayload = jwtDecode(idToken);
          setUserName(decoded.name || decoded.email || "User");
          setIsAuthenticated(true);
        } else {
          setIsAuthenticated(false);
        }
      } catch (e) {
        setIsAuthenticated(false);
      }
    }
    async function fetchClientId() {
      try {
        const clientId: string = "917256818414-pcsi1favsuki4crrmd5st51ebp6ghl3g.apps.googleusercontent.com";
        setGoogleClientId(clientId);
      } catch (e) {
        setGoogleClientId("");
      }
    }
    checkAuth();
    fetchClientId();
  }, []);

  // Google OAuth handler (OOB flow)
  const handleGoogleAuth = async () => {
    setAuthError(null);
    if (!googleClientId) {
      alert("Google Client ID not set");
      return;
    }
    // OOB redirect URI
    const redirectUri = "urn:ietf:wg:oauth:2.0:oob";
    const scope = encodeURIComponent("openid email profile https://www.googleapis.com/auth/drive");
    const url = `https://accounts.google.com/o/oauth2/v2/auth?client_id=${googleClientId}&redirect_uri=${encodeURIComponent(redirectUri)}&response_type=code&scope=${scope}&access_type=offline&prompt=consent`;
    await openUrl(url);
    setShowCodeInput(true);
  };

  // Exchange code for tokens
  const handleExchangeCode = async () => {
    setIsExchanging(true);
    setAuthError(null);
    try {
      const redirectUri = "urn:ietf:wg:oauth:2.0:oob";
      const params = new URLSearchParams({
        code: authCode,
        client_id: googleClientId,
        client_secret: "",
        redirect_uri: redirectUri,
        grant_type: "authorization_code",
      });
      const resp = await fetch("https://oauth2.googleapis.com/token", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: params.toString(),
      });
      if (!resp.ok) {
        throw new Error("Failed to exchange code: " + (await resp.text()));
      }
      const tokens: GoogleTokens = await resp.json();
      await invoke("save_google_tokens", { tokens });
      // Decode and set user
      const decoded: IdTokenPayload = jwtDecode(tokens.id_token);
      setUserName(decoded.name || decoded.email || "User");
      setIsAuthenticated(true);
      setShowCodeInput(false);
      setAuthCode("");
    } catch (e: any) {
      setAuthError(e.message || "Unknown error");
    } finally {
      setIsExchanging(false);
    }
  };

  const handleLogout = () => {
    setIsAuthenticated(false);
    setUserName("");
    setShowCodeInput(false);
    setAuthCode("");
    setAuthError(null);
  };

  return (
    <div className="flex flex-col bg-gray-50 dark:bg-gray-900 h-screen overflow-hidden">
      {/* Navbar / App Bar */}
      <div className="sticky top-0 z-20 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 shadow-sm flex items-center h-14 px-4 gap-2 shrink-0">
        {/* App Name/Logo */}
        <div className="flex items-center font-bold text-lg text-blue-700 dark:text-blue-300 mr-6 select-none whitespace-nowrap">
          <svg className="w-6 h-6 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="10" strokeWidth="2" />
            <path d="M8 12l2 2 4-4" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <Link to="/" className="hover:text-blue-800 dark:hover:text-blue-200">
            SWE Reviewer
          </Link>
        </div>

        {/* Navigation Links */}
        {isAuthenticated && (
          <nav className="flex gap-6">
           <Link
              to="/report-checker"
              className={`px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                location.pathname === '/report-checker'
                  ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                  : 'text-gray-600 dark:text-gray-300 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              Report Checker
            </Link>
            <Link
              to="/gdrive-editor"
              className={`px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                location.pathname === '/' || location.pathname === '/gdrive-editor'
                  ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                  : 'text-gray-600 dark:text-gray-300 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              gDrive Editor
            </Link>
            
          </nav>
        )}

        {/* Centered file name if open */}
        <div className="flex-1 flex justify-center">

        </div>

        {/* Greeting on right if authenticated */}
        {isAuthenticated && (
          <button
            onClick={() => setIsSettingsOpen(true)}
            className="text-base font-medium text-blue-700 dark:text-blue-300 whitespace-nowrap hover:text-blue-800 dark:hover:text-blue-200 transition-colors cursor-pointer"
          >
            Hello {userName}
          </button>
        )}
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {isAuthenticated === null ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-gray-500 text-lg">Checking authentication...</div>
          </div>
        ) : isAuthenticated ? (
          <div className="flex-1 overflow-hidden p-4">
            {children}
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center">
            <div className="text-center mb-8">
              <h1 className="text-3xl font-bold text-gray-900 dark:text-white mb-4">
                Welcome to SWE Reviewer
              </h1>
              <p className="text-gray-600 dark:text-gray-400 mb-8">
                Please sign in with Google to access the application
              </p>
            </div>
            
            <button
              className="px-6 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-lg font-semibold shadow transition-colors"
              onClick={handleGoogleAuth}
            >
              Sign in with Google
            </button>
            
            {showCodeInput && (
              <div className="mt-6 flex flex-col items-center gap-2 w-full max-w-md">
                <label className="text-gray-700 dark:text-gray-200 font-medium">
                  Paste the code from Google here:
                </label>
                <input
                  className="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
                  value={authCode}
                  onChange={e => setAuthCode(e.target.value)}
                  placeholder="Enter code"
                  disabled={isExchanging}
                />
                <button
                  className="mt-2 px-4 py-2 bg-green-600 hover:bg-green-700 text-white rounded font-semibold disabled:opacity-50 transition-colors"
                  onClick={handleExchangeCode}
                  disabled={!authCode || isExchanging}
                >
                  {isExchanging ? "Exchanging..." : "Submit Code"}
                </button>
                {authError && <div className="text-red-600 mt-2">{authError}</div>}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Settings Drawer */}
      <SettingsDrawer
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        onLogout={handleLogout}
      />
    </div>
  );
}
