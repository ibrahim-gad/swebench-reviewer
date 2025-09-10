import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SettingsDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  onLogout: () => void;
  isAuthenticated: boolean;
}

export default function SettingsDrawer({ isOpen, onClose, onLogout, isAuthenticated }: SettingsDrawerProps) {
  const [googleClientSecret, setGoogleClientSecret] = useState("");
  const [geminiApiKey, setGeminiApiKey] = useState("");
  const [openaiApiKey, setOpenaiApiKey] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [tempDirSize, setTempDirSize] = useState<number>(0);
  const [isClearingTemp, setIsClearingTemp] = useState(false);

  // Load settings when drawer opens
  useEffect(() => {
    if (isOpen) {
      loadSettings();
      loadTempDirSize();
    }
  }, [isOpen]);

  const loadSettings = async () => {
    try {
      const clientSecret = await invoke<string>("load_setting", { key: "google_client_secret" });
      const geminiKey = await invoke<string>("load_setting", { key: "gemini_api_key" });
      const openaiKey = await invoke<string>("load_setting", { key: "openai_api_key" });
      setGoogleClientSecret(clientSecret || "");
      setGeminiApiKey(geminiKey || "");
      setOpenaiApiKey(openaiKey || "");
    } catch (error) {
      console.error("Failed to load settings:", error);
    }
  };

  const loadTempDirSize = async () => {
    try {
      const size = await invoke<number>("get_temp_dir_size");
      setTempDirSize(size);
    } catch (error) {
      console.error("Failed to load temp directory size:", error);
    }
  };

  const handleSave = async () => {
    setIsLoading(true);
    setSaveSuccess(false);
    
    try {
      await invoke("save_setting", { 
        key: "google_client_secret", 
        value: googleClientSecret 
      });
      await invoke("save_setting", { 
        key: "gemini_api_key", 
        value: geminiApiKey 
      });
      await invoke("save_setting", { 
        key: "openai_api_key", 
        value: openaiApiKey 
      });
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2000);
    } catch (error) {
      console.error("Failed to save settings:", error);
      alert("Failed to save settings: " + error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    try {
      await invoke("logout");
      onLogout();
      onClose();
    } catch (error) {
      console.error("Failed to logout:", error);
      alert("Failed to logout: " + error);
    }
  };

  const handleClearTemp = async () => {
    if (!confirm("Are you sure you want to clear the temporary directory? This will remove all cached files.")) {
      return;
    }

    setIsClearingTemp(true);
    try {
      await invoke("clear_temp_dir");
      setTempDirSize(0);
      alert("Temporary directory cleared successfully!");
    } catch (error) {
      console.error("Failed to clear temp directory:", error);
      alert("Failed to clear temp directory: " + error);
    } finally {
      setIsClearingTemp(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  };

  return (
    <>
      {/* Backdrop */}
      {isOpen && (
        <div
          className="fixed inset-0 bg-black bg-opacity-50 z-40 transition-opacity"
          onClick={onClose}
        />
      )}

      {/* Drawer */}
      <div
        className={`fixed top-0 right-0 h-full w-96 bg-white dark:bg-gray-800 shadow-xl z-50 transform transition-transform duration-300 ease-in-out flex flex-col ${
          isOpen ? "translate-x-0" : "translate-x-full"
        }`}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
            Settings
          </h2>
          <button
            onClick={onClose}
            className="p-2 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="flex flex-col flex-1 min-h-0">
          <div className="flex-1 p-6 space-y-6 overflow-y-auto">
            {/* Google OAuth Client Secret */}
            <div className="space-y-3">
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                Google OAuth Client Secret
              </label>
              <input
                type="password"
                value={googleClientSecret}
                onChange={(e) => setGoogleClientSecret(e.target.value)}
                placeholder="Enter your Google OAuth Client Secret"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Required for Google Drive authentication. Keep this secret secure.
              </p>
            </div>

            {/* Google Gemini API Key */}
            <div className="space-y-3">
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                Google Gemini API Key
              </label>
              <input
                type="password"
                value={geminiApiKey}
                onChange={(e) => setGeminiApiKey(e.target.value)}
                placeholder="Enter your Gemini API key"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
              <p className="text-xs text-gray-500 dark:text-gray-400">
                This key is stored locally and used for AI-powered features.
              </p>
            </div>

            {/* OpenAI API Key */}
            <div className="space-y-3">
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                OpenAI API Key
              </label>
              <input
                type="password"
                value={openaiApiKey}
                onChange={(e) => setOpenaiApiKey(e.target.value)}
                placeholder="Enter your OpenAI API key"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
              <p className="text-xs text-gray-500 dark:text-gray-400">
                This key is stored locally and used for AI-powered features with GPT-4o.
              </p>
            </div>

            {/* Save Button */}
            <div className="flex flex-col space-y-2">
              <button
                onClick={handleSave}
                disabled={isLoading}
                className="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white rounded-md font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
              >
                {isLoading ? "Saving..." : "Save Settings"}
              </button>
              
              {saveSuccess && (
                <div className="text-green-600 dark:text-green-400 text-sm text-center">
                  Settings saved successfully!
                </div>
              )}
            </div>
          </div>

          {/* Footer with Logout and Clear Temp - Fixed at bottom */}
          {isAuthenticated && (
            <div className="p-6 border-t border-gray-200 dark:border-gray-700 flex-shrink-0 space-y-3">
              <button
                onClick={handleClearTemp}
                disabled={isClearingTemp || tempDirSize === 0}
                className="w-full px-4 py-2 bg-orange-600 hover:bg-orange-700 disabled:bg-orange-400 text-white rounded-md font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-orange-500 focus:ring-offset-2"
              >
                {isClearingTemp ? "Clearing..." : `Clear Temp (${formatBytes(tempDirSize)})`}
              </button>
              <button
                onClick={handleLogout}
                className="w-full px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-md font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2"
              >
                Logout
              </button>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
