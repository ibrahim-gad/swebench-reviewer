import { useState } from "react";
import DriveTextEditor from "../components/DriveTextEditor";

export default function GDriveEditorPage() {
  const [openedFileName, setOpenedFileName] = useState<string | null>(null);

  return (
    <div className="flex flex-col items-center justify-center w-full">
      <div className="w-full max-w-7xl">
        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-6 shadow-sm">
          <DriveTextEditor onFileNameChange={setOpenedFileName} />
        </div>
      </div>
    </div>
  );
}
