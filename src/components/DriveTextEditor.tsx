import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import MonacoEditor from '@monaco-editor/react';

interface DriveTextEditorProps {
  onFileNameChange?: (name: string) => void;
}

export default function DriveTextEditor({ onFileNameChange }: DriveTextEditorProps) {
  const [link, setLink] = useState('');
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [editorKey, setEditorKey] = useState(0); // for resetting editor
  const [editorLanguage, setEditorLanguage] = useState<'plaintext' | 'json'>('plaintext');
  const [fileName, setFileName] = useState<string | null>(null);
  // For JSON key editing
  const [isJsonObject, setIsJsonObject] = useState(false);
  const [jsonKeys, setJsonKeys] = useState<string[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [showEditModal, setShowEditModal] = useState(false);
  const [editValue, setEditValue] = useState<string>("");

  const handleDownload = async () => {
    setLoading(true);
    setError('');
    setSuccess('');
    try {
      const result = await invoke<any>('download_drive_file', { link });
      setContent(result.content);
      setFileName(result.name);
      if (onFileNameChange) onFileNameChange(result.name);
      // Detect JSON by link or content
      let isJson = false;
      let isObj = false;
      let keys: string[] = [];
      if (link.trim().toLowerCase().endsWith('.json')) {
        setEditorLanguage('json');
        try {
          const parsed = JSON.parse(result.content);
          isJson = true;
          if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
            isObj = true;
            keys = Object.keys(parsed);
          }
        } catch {}
      } else {
        try {
          const parsed = JSON.parse(result.content);
          setEditorLanguage('json');
          isJson = true;
          if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
            isObj = true;
            keys = Object.keys(parsed);
          }
        } catch {
          setEditorLanguage('plaintext');
        }
      }
      setIsJsonObject(isObj);
      setJsonKeys(keys);
      setEditorKey((k) => k + 1); // reset editor
    } catch (e: any) {
      setError(e?.toString() || 'Failed to download file');
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    setLoading(true);
    setError('');
    setSuccess('');
    try {
      await invoke('upload_drive_file', { link, content });
      setSuccess('File saved successfully!');
    } catch (e: any) {
      setError(e?.toString() || 'Failed to save file');
    } finally {
      setLoading(false);
    }
  };

  // Helper to get and set value for selected key
  const getSelectedValue = () => {
    try {
      const parsed = JSON.parse(content);
      if (selectedKey && parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        // If value is a string, show as is, else show pretty JSON
        const value = parsed[selectedKey];
        if (typeof value === 'string') return value;
        return value !== undefined ? JSON.stringify(value, null, 2) : '';
      }
    } catch {}
    return '';
  };

  const handleOpenEditModal = () => {
    setEditValue(getSelectedValue());
    setShowEditModal(true);
  };

  const handleSaveValue = () => {
    try {
      const parsed = JSON.parse(content);
      if (selectedKey && parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        // Try to parse as JSON, but if fails, treat as string
        let newValue: any = editValue;
        try {
          newValue = JSON.parse(editValue);
        } catch {
          // keep as string
        }
        parsed[selectedKey] = newValue;
        setContent(JSON.stringify(parsed, null, 2));
        setShowEditModal(false);
      }
    } catch (e) {
      alert('Invalid JSON value');
    }
  };

  return (
    <div className="w-full max-w-6xl mx-auto">
      <div className="mb-2 flex gap-2 items-center">
        <h2 className="text-xl font-bold whitespace-nowrap">Edit Google Drive Text File</h2>
        <input
          type="text"
          className="flex-1 border rounded px-2 py-1"
          placeholder="Paste Google Drive file link here"
          value={link}
          onChange={e => setLink(e.target.value)}
          disabled={loading}
        />
        <button
          className="bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
          onClick={handleDownload}
          disabled={loading || !link}
        >
          {loading ? 'Loading...' : 'Edit'}
        </button>
      </div>
      {error && <div className="text-red-600 mb-2">{error}</div>}
      {success && <div className="text-green-600 mb-2">{success}</div>}
      <div className="mb-4 max-h-[500px] h-[500px] border rounded">
        <MonacoEditor
          key={editorKey}
          height="100%"
          defaultLanguage={editorLanguage}
          language={editorLanguage}
          value={content}
          onChange={v => setContent(v ?? '')}
          options={{ readOnly: loading }}
        />
      </div>
      <div className="flex items-center gap-4">
        <button
          className="bg-green-500 text-white px-4 py-2 rounded hover:bg-green-600 disabled:opacity-50 transition-colors"
          onClick={handleSave}
          disabled={loading || !content || !link}
        >
          {loading ? 'Saving...' : 'Save'}
        </button>
        {isJsonObject && (
          <div className="flex items-center gap-2">
            <select
              className="border rounded px-2 py-1"
              value={selectedKey ?? ''}
              onChange={e => setSelectedKey(e.target.value)}
            >
              <option value="" disabled>Select key</option>
              {jsonKeys.map(k => (
                <option key={k} value={k}>{k}</option>
              ))}
            </select>
            <button
              className="bg-blue-500 text-white px-3 py-1 rounded disabled:opacity-50 hover:bg-blue-600 transition-colors"
              disabled={!selectedKey}
              onClick={handleOpenEditModal}
            >
              Edit value
            </button>
          </div>
        )}
      </div>
      
      {/* Modal for editing value */}
      {showEditModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-40">
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow-lg p-8 w-3/4 h-3/4 flex flex-col">
            <div className="mb-4 font-semibold text-lg">
              Edit value for key: <span className="text-blue-600">{selectedKey}</span>
            </div>
            <textarea
              className="flex-1 w-full border rounded p-3 mb-4 text-gray-900 dark:text-gray-100 bg-white dark:bg-gray-900 resize-none"
              style={{ minHeight: '300px', maxHeight: '100%' }}
              value={editValue}
              onChange={e => setEditValue(e.target.value)}
            />
            <div className="flex justify-end gap-2 mt-2">
              <button
                className="px-4 py-2 bg-gray-300 dark:bg-gray-700 rounded hover:bg-gray-400 dark:hover:bg-gray-600 transition-colors"
                onClick={() => setShowEditModal(false)}
              >
                Cancel
              </button>
              <button
                className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 transition-colors"
                onClick={handleSaveValue}
              >
                Save value
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
